use clap::Parser;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::fs::File;
use std::io::Read;

pub fn get() -> Config {
    let args = Args::parse();

    let config: Config = {
        let mut f = File::open(&args.config).unwrap_or_else(|_| panic!("Missing config file {}", args.config));

        let mut buf = Vec::new();
        f.read_to_end(&mut buf).expect("Error reading config");
        toml::from_slice(&buf).expect("Error deserializing config")
    };

    config
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, default_value = "./vedirect2mqtt.conf")]
    pub config: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub mqtt: MqttConfig,
    pub device: DeviceConfig,
    pub publish: PublishConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MqttConfig {
    pub host: String,
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_topic")]
    pub topic: String,
}

fn default_mqtt_port() -> u16 {
    1883
}

fn default_client_id() -> String {
    "vedirect2mqtt".to_string()
}

fn default_topic() -> String {
    "vedirect2mqtt".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeviceConfig {
    #[serde(default = "default_device_path")]
    pub path: String,
}

fn default_device_path() -> String {
    "/dev/ttyUSB0".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PublishConfig {
    #[serde(default = "default_publish_interval")]
    pub interval: u64,
}

fn default_publish_interval() -> u64 {
    60
}
