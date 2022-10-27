mod data;

use clap::Parser;
use data::Message;
use log::{error, info};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_json::Value;
use serialport::SerialPort;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tokio::{task, time};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    device: String,

    #[arg(long)]
    mqtt_host: String,

    #[arg(long, default_value_t = 1883)]
    mqtt_port: u16,

    #[arg(long)]
    mqtt_username: String,

    #[arg(long)]
    mqtt_password: String,

    #[arg(long, default_value = "vedirect2mqtt")]
    mqtt_client_id: String,

    #[arg(long)]
    mqtt_topic: String,

    #[arg(long, default_value_t = 60)]
    interval: u64,
}

type DataLock = Arc<Mutex<Option<Value>>>;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // setup logging
    let env = env_logger::Env::new().filter_or("LOG", "info");
    env_logger::Builder::from_env(env).init();

    let args = Args::parse();

    log::info!("Starting");

    let p = serialport::new(args.device.clone(), 19200)
        .stop_bits(serialport::StopBits::One)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .timeout(Duration::from_millis(5000))
        .open()
        .expect("Failed to open serial connection");

    let data = Arc::new(Mutex::new(None));

    let h = start_receiver(p, Arc::clone(&data), args.clone());

    start_mqtt(data, args).await.unwrap();

    h.join().unwrap();
}

async fn start_mqtt(data: DataLock, args: Args) -> Result<(), tokio::io::Error> {
    let mut mqttoptions = MqttOptions::new(args.mqtt_client_id, args.mqtt_host, args.mqtt_port);
    mqttoptions.set_credentials(args.mqtt_username, args.mqtt_password);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_connection_timeout(5);
    mqttoptions.set_clean_session(true);

    let clear_topic = args.mqtt_topic.clone() + "/clear";

    log::info!("mqtt client connecting ...");

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe(&clear_topic, QoS::AtMostOnce).await.unwrap();

    log::info!("mqtt client connected");

    task::spawn(async move {
        loop {
            time::sleep(Duration::from_millis(500)).await;

            let value = { data.lock().expect("Unable to obtain lock").take() };

            if let Some(value) = value {
                let json = serde_json::to_string(&value).map_err(|e| log::warn!("Error serializing json: {}", e)).ok();

                if json.is_none() {
                    continue;
                }

                client
                    .publish(args.mqtt_topic.clone(), QoS::AtLeastOnce, false, json.unwrap())
                    .await
                    .map_err(|e| log::warn!("Error publishine message: {}", e))
                    .ok();
            }
        }
    });

    while let Ok(notification) = eventloop.poll().await {
        if let Event::Incoming(Packet::Publish(packet)) = notification {
            if packet.topic == clear_topic {
                log::info!("Received 'clear' packet: {:?}", packet.payload);
            }
        }
    }

    Ok(())
}

fn start_receiver(port: Box<dyn SerialPort>, data: DataLock, args: Args) -> JoinHandle<()> {
    let mut reader = SerialReader::new(port);

    thread::spawn(move || {
        info!("Started receiver thread");

        let interval = Duration::from_secs(args.interval);
        let mut collector = data::Collector::new();
        let mut publish_message_at = Instant::now() + Duration::from_secs(15);

        loop {
            let now = Instant::now();
            match reader.read() {
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
enum ReaderState {
    Idle,
    FieldName,
    FieldValue,
    Checksum,
    HexRecord,
}
struct SerialReader {
    port: Box<dyn serialport::SerialPort>,
    buf: [u8; 1024],
    buf_start: usize,
    buf_end: usize,
    field_name: String,
    field_value: String,
}

impl SerialReader {
    fn new(port: Box<dyn serialport::SerialPort>) -> SerialReader {
        SerialReader {
            port,
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
        // let mut checksum_error_count = 0;
        let mut state = ReaderState::Idle;
        let mut state_after_hex = ReaderState::Idle;
        self.field_name.clear();
        self.field_value.clear();

        loop {
            let b = self.next_byte()?;

            if b == b':' && state != ReaderState::Checksum {
                state_after_hex = state;
                state = ReaderState::HexRecord;
                // log::warn!("Detected hex record");
            }

            if state != ReaderState::HexRecord {
                checksum = checksum.wrapping_add(b);
            }

            match &state {
                ReaderState::Idle => {
                    // wait for \n marking start of record
                    if b == b'\n' {
                        state = ReaderState::FieldName;
                    }
                }
                ReaderState::FieldName => {
                    // 	read field name terminated by \t
                    match b {
                        b'\t' => {
                            if self.field_name == "Checksum" {
                                state = ReaderState::Checksum;
                            } else {
                                state = ReaderState::FieldValue;
                            }
                        }
                        c if c.is_ascii() => self.field_name.push(c as char),
                        c => log::warn!("Unexpected non-ascii character '{}'", c),
                    };
                }
                ReaderState::FieldValue => {
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
                            state = ReaderState::FieldName;
                        }
                        c if c.is_ascii() => self.field_value.push(c as char),
                        c => log::warn!("Unexpected non-ascii character '{}'", c),
                    };
                }
                ReaderState::Checksum => {
                    // checksum byte has jsut been received and added to checksum value, which should now be 0
                    if checksum == 0 {
                        return Ok(message);
                    } else {
                        return Err(ReadError::ChecksumError);
                    }
                }
                ReaderState::HexRecord => {
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
                self.buf_end = match self.port.read(&mut self.buf[..]) {
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
