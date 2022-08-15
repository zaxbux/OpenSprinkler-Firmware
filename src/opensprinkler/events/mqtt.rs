use core::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};
extern crate paho_mqtt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub enabled: bool,
    pub version: u32,
    /// Broker
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Use TLS
    pub tls: bool,

    pub root_topic: String,
    pub availability_topic: String,
    pub offline_payload: String,
    pub online_payload: String,

    /// Enabled events
    pub events: super::EventsEnabled,
}

impl Config {
    const PROTOCOL_TCP: &'static str = "tcp";
    const PROTOCOL_SSL: &'static str = "tcp";
    const PROTOCOL_WS: &'static str = "ws";
    const PROTOCOL_WSS: &'static str = "wss";

    pub fn protocol(&self) -> &'static str {
        match self.tls {
            false => Self::PROTOCOL_TCP,
            true => Self::PROTOCOL_SSL,
        }
    }

    pub fn uri(&self) -> Option<String> {
        if let Some(ref host) = self.host {
            return Some(format!("{}://{}:{}", self.protocol(), host, self.port));
        }

        None
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            version: paho_mqtt::MQTT_VERSION_3_1_1,
            host: None,
            port: 1883,
            username: None,
            password: None,
            tls: false,
            root_topic: String::from("opensprinkler"),
            availability_topic: String::from("availability"),
            offline_payload: String::from("offline"),
            online_payload: String::from("online"),

            events: super::EventsEnabled::default(),
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.uri())
    }
}

/// Program Start
#[derive(Serialize, Deserialize)]
pub struct ProgramSchedPayload {
    pub program_index: usize,
    pub program_name: String,
    /* pub manual: bool, */
    pub water_scale: f32,
}

/// Sensor
#[derive(Serialize, Deserialize)]
pub struct BinarySensorPayload {
    pub state: bool,
}

/// Rain Delay state
#[derive(Serialize, Deserialize)]
pub struct RainDelayPayload {
    pub state: bool,
}

#[derive(Serialize, Deserialize)]
pub struct FlowSensorPayload {
    pub count: i64,
    pub volume: f64,
}

/// Weather Update
#[derive(Serialize, Deserialize)]
pub struct WeatherUpdatePayload {
    pub scale: Option<f32>,
    pub external_ip: Option<IpAddr>,
}

/// Controller reboot
#[derive(Serialize, Deserialize)]
pub struct RebootPayload {
    pub state: String,
}

/// Station On/Off
#[derive(Serialize, Deserialize)]
pub struct StationPayload {
    pub state: bool,
    pub duration: Option<i64>,
    pub flow: Option<f64>,
}

pub trait MqttEvent {
    fn mqtt_topic(&self) -> String;
    fn mqtt_payload(&self) -> serde_json::Result<String>;
}

impl MqttEvent for super::ProgramStartEvent {
    fn mqtt_topic(&self) -> String {
        String::from("program")
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&ProgramSchedPayload {
            program_index: self.program_index,
            program_name: self.program_name.clone(),
            /* manual: self.manual, */
            water_scale: self.water_scale,
        })
    }
}

impl MqttEvent for super::BinarySensorEvent {
    fn mqtt_topic(&self) -> String {
        format!("sensor{}", self.index)
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&BinarySensorPayload { state: self.state })
    }
}

impl MqttEvent for super::FlowSensorEvent {
    fn mqtt_topic(&self) -> String {
        String::from("sensor/flow")
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&FlowSensorPayload { count: self.count, volume: self.volume })
    }
}

impl MqttEvent for super::WeatherUpdateEvent {
    fn mqtt_topic(&self) -> String {
        String::from("weather")
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&WeatherUpdatePayload {
            scale: self.scale,
            external_ip: self.external_ip,
        })
    }
}

impl MqttEvent for super::RebootEvent {
    fn mqtt_topic(&self) -> String {
        String::from("system")
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&BinarySensorPayload { state: self.state })
    }
}

impl MqttEvent for super::StationEvent {
    fn mqtt_topic(&self) -> String {
        format!("station/{}", self.station_index)
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&StationPayload {
            state: self.state,
            duration: self.duration,
            flow: self.flow,
        })
    }
}

impl MqttEvent for super::RainDelayEvent {
    fn mqtt_topic(&self) -> String {
        String::from("raindelay")
    }

    fn mqtt_payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&RainDelayPayload { state: self.state })
    }
}

pub(super) trait PublishMqttMessage {
    fn mqtt_publish<E: super::Event>(&self, config: &Config, event: &E) -> Result<(), serde_json::Error>;
}

impl PublishMqttMessage for super::Events {
    fn mqtt_publish<E: super::Event>(&self, config: &Config, event: &E) -> Result<(), serde_json::Error> {
        self.mqtt_client
            .publish(paho_mqtt::MessageBuilder::new().topic(format!("{}/{}", config.root_topic, event.mqtt_topic())).payload(event.mqtt_payload()?).finalize());

        Ok(())
    }
}
