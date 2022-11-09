mod config;
mod data;

use config::Config;
use data::Message;
use log::{error, info};
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions, Outgoing, Packet, QoS};
use serde_json::Value;
use serialport::SerialPort;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tokio::{task, time};

type DataLock = Arc<Mutex<Option<Value>>>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // setup logging
    let env = env_logger::Env::new().filter_or("LOG", "info");
    env_logger::Builder::from_env(env).init();

    let config = config::get();

    log::info!("Starting");

    let port = serialport::new(config.device.path.clone(), 19200)
        .stop_bits(serialport::StopBits::One)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .timeout(Duration::from_millis(5000))
        .open()
        .expect("Failed to open serial connection");

    let data = Arc::new(Mutex::new(None));
    let reset = Arc::new(AtomicBool::new(false));

    let h = start_receiver(port, Arc::clone(&data), Arc::clone(&reset), config.clone());

    start_mqtt(data, reset, config).await.unwrap();

    h.join().unwrap();
}

async fn start_mqtt(data: DataLock, reset: Arc<AtomicBool>, config: Config) -> Result<(), tokio::io::Error> {
    let mut mqttoptions = MqttOptions::new(config.mqtt.client_id, config.mqtt.host, config.mqtt.port);
    mqttoptions.set_credentials(config.mqtt.username.unwrap(), config.mqtt.password.unwrap());
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_connection_timeout(5);
    mqttoptions.set_clean_session(true);

    let clear_topic = config.mqtt.topic.clone() + "/clear";

    log::info!("MQTT connecting.");

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let loop_client = client.clone();
    task::spawn(async move {
        loop {
            time::sleep(Duration::from_millis(500)).await;

            let value = { data.lock().expect("Unable to obtain lock").take() };

            if let Some(value) = value {
                let json = serde_json::to_string(&value).map_err(|e| log::warn!("Error serializing json: {}", e)).ok();

                if json.is_none() {
                    continue;
                }

                loop_client
                    .publish(config.mqtt.topic.clone(), QoS::AtLeastOnce, false, json.unwrap())
                    .await
                    .map_err(|e| log::warn!("Error publishing message: {}", e))
                    .ok();
            }
        }
    });

    loop {
        let event = eventloop.poll().await;

        match event {
            Ok(Event::Incoming(Packet::Publish(packet))) => {
                if packet.topic == clear_topic {
                    log::info!("Received 'clear' packet.");
                    reset.store(true, Ordering::Relaxed);
                }
            }
            Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                log::info!("MQTT connected.  Subscribing");
                client.subscribe(&clear_topic, QoS::AtMostOnce).await.unwrap();
            }
            Ok(Event::Incoming(Incoming::PingReq)) => (),
            Ok(Event::Incoming(Incoming::PingResp)) => (),
            Ok(Event::Outgoing(Outgoing::PingResp)) => (),
            Ok(Event::Outgoing(Outgoing::PingReq)) => (),
            Ok(Event::Outgoing(Outgoing::Subscribe(_))) => (),
            Ok(Event::Incoming(Incoming::SubAck(s))) => {
                log::info!("Subscription acknowledge: {:?}", s.return_codes);
            }
            Ok(Event::Outgoing(Outgoing::Publish(_))) => (),
            Ok(Event::Incoming(Incoming::PubAck(_))) => (),
            Err(ConnectionError::Io(_)) => {
                log::info!("MQTT connection error. Waiting for 2 secs before trying again");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(ConnectionError::MqttState(rumqttc::StateError::Io(e))) if e.kind() == std::io::ErrorKind::ConnectionAborted => {
                log::info!("MQTT connection aborted.  Waiting for 2 secs before trying again");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(ConnectionError::ConnectionRefused(reason)) => {
                log::info!("MQTT connection refused: {:?}.  Aborting.", reason);
                return Ok(());
            }
            other => {
                log::info!("Other: {:?}", other);
            }
        }
    }
}

fn start_receiver(port: Box<dyn SerialPort>, data: DataLock, reset: Arc<AtomicBool>, config: Config) -> JoinHandle<()> {
    let mut decoder = SerialDecoder::new(port);

    thread::spawn(move || {
        info!("Started receiver thread");

        let interval = Duration::from_secs(config.publish.interval);
        let mut collector = data::Collector::new();
        let mut publish_message_at = Instant::now() + interval;

        loop {
            let now = Instant::now();
            match decoder.read() {
                Ok(msg) => {
                    collector.collect(msg);
                }
                Err(ReadError::ChecksumError) => {
                    collector.increment_checksum_error();
                    log::warn!("Checksum error");
                }
                Err(ReadError::Timeout) => {
                    error!("Timeout error");
                }
                Err(e) => {
                    error!("LineReader error: {:?}", e);
                }
            }

            if now >= publish_message_at && collector.message_count() > 0 {
                log::debug!("Receiver publishing message");

                if reset.swap(false, Ordering::Relaxed) {
                    // FIXME: this reset means count=0 and no value gets transmitted.  How to guarantee the 0 gets sent ?
                    collector.reset_totals();
                }

                // log::info!("Collector {:?}", collector);
                let msg = collector.build_json_message();
                // log::info!("Collector output message {:?}", msg);
                {
                    let mut guard = data.lock().unwrap();
                    *guard = Some(msg);
                }

                publish_message_at = now + interval;
                collector = data::Collector::next(collector);
                // log::warn!("******** NEW CYCLE **********")
            }
        }
    })
}

#[derive(Clone, PartialEq)]
enum DecoderState {
    Idle,
    FieldName,
    FieldValue,
    Checksum,
    HexRecord,
}

struct SerialDecoder<T> {
    reader: T,
    buf: [u8; 1024],
    buf_start: usize,
    buf_end: usize,
    field_name: String,
    field_value: String,
}

impl<T> SerialDecoder<T>
where
    T: std::io::Read,
{
    fn new(reader: T) -> SerialDecoder<T> {
        SerialDecoder {
            reader,
            buf: [0; 1024],
            buf_start: 0,
            buf_end: 0,
            field_name: String::with_capacity(128),
            field_value: String::with_capacity(128),
        }
    }
    fn read(&mut self) -> Result<Message, ReadError> {
        let mut message = Message::new();
        let mut checksum: u8 = 0;
        let mut state = DecoderState::Idle;
        let mut state_after_hex = DecoderState::Idle;
        self.field_name.clear();
        self.field_value.clear();

        loop {
            let b = self.next_byte()?;

            if b == b':' && state != DecoderState::Checksum {
                state_after_hex = state;
                state = DecoderState::HexRecord;
                // log::warn!("Detected hex record");
            }

            if state != DecoderState::HexRecord {
                checksum = checksum.wrapping_add(b);
            }

            match &state {
                DecoderState::Idle => {
                    // wait for \n marking start of record
                    if b == b'\n' {
                        state = DecoderState::FieldName;
                    }
                }
                DecoderState::FieldName => {
                    // 	read field name terminated by \t
                    match b {
                        b'\t' => {
                            if self.field_name == "Checksum" {
                                state = DecoderState::Checksum;
                            } else {
                                state = DecoderState::FieldValue;
                            }
                        }
                        c if c.is_ascii() => self.field_name.push(c as char),
                        c => log::warn!("Unexpected non-ascii character '{}'", c),
                    };
                }
                DecoderState::FieldValue => {
                    // read field value terminated by \n
                    match b {
                        b'\r' => { /* ignore */ }
                        b'\n' => {
                            message
                                .set_field(&self.field_name, &self.field_value)
                                .map_err(|e| log::warn!("Error handling field: {}", e))
                                .ok();
                            self.field_name.clear();
                            self.field_value.clear();
                            state = DecoderState::FieldName;
                        }
                        c if c.is_ascii() => self.field_value.push(c as char),
                        c => log::warn!("Unexpected non-ascii character '{}'", c),
                    };
                }
                DecoderState::Checksum => {
                    // checksum byte has jsut been received and added to checksum value, which should now be 0
                    if checksum == 0 {
                        return Ok(message);
                    } else {
                        return Err(ReadError::ChecksumError);
                    }
                }
                DecoderState::HexRecord => {
                    // ignore everything until \n char
                    if b == b'\n' {
                        state = state_after_hex.clone();
                    }
                }
            }
        }
    }

    fn next_byte(&mut self) -> Result<u8, ReadError> {
        loop {
            if self.buf_start < self.buf_end {
                // more bytes in buffer - return next byte from buffer
                let result = self.buf[self.buf_start];
                self.buf_start += 1;
                return Ok(result);
            } else {
                // load more bytes into buffer
                self.buf_start = 0;
                self.buf_end = match self.reader.read(&mut self.buf[..]) {
                    Ok(read_count) => read_count,
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        return Err(ReadError::Timeout);
                    }
                    Err(e) => return Err(ReadError::Other(format!("Error reading from device: {}", e))),
                }
            }
        }
    }
}

#[derive(Debug)]
enum ReadError {
    Timeout,
    ChecksumError,
    Other(String),
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_simple_read() {
        let mut stream = Vec::new();
        append_packet(&mut stream, b"\nPID\t42\nChecksum\t");

        let mut decoder = SerialDecoder::new(stream.as_slice());
        let actual = decoder.read().unwrap();
        let expected = Message {
            product_id: Some("42".to_string()),
            ..Default::default()
        };
        assert_eq!(actual, expected);
    }

    fn append_packet(stream: &mut Vec<u8>, data: &[u8]) {
        stream.extend(data);
        stream.push(compute_checksum(data));
    }

    fn compute_checksum(data: &[u8]) -> u8 {
        0u8.wrapping_sub(data.into_iter().fold(0u8, |acc, val| acc.wrapping_add(*val)))
    }
}
