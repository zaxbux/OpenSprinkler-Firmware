use core::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::opensprinkler::config;

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
    pub events: config::EventsEnabled,
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

            events: config::EventsEnabled::default(),
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
    pub program_index: Option<usize>,
    pub program_name: Option<String>,
    pub water_scale: Option<f32>,
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
    pub count: i32,
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
    fn topic(&self) -> String;
    fn payload(&self) -> serde_json::Result<String>;
}

impl MqttEvent for super::ProgramStartEvent {
    fn topic(&self) -> String {
        String::from("program")
    }

    fn payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&ProgramSchedPayload {
            program_index: self.program_index,
            program_name: self.program_name.clone(),
            water_scale: self.water_scale,
        })
    }
}

impl MqttEvent for super::BinarySensorEvent {
    fn topic(&self) -> String {
        format!("sensor{}", self.index)
    }

    fn payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&BinarySensorPayload { state: self.state })
    }
}

impl MqttEvent for super::FlowSensorEvent {
    fn topic(&self) -> String {
        String::from("sensor/flow")
    }

    fn payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&FlowSensorPayload { count: self.count, volume: self.volume })
    }
}

impl MqttEvent for super::WaterScaleChangeEvent {
    fn topic(&self) -> String {
        "water_scale".into()
    }

    fn payload(&self) -> serde_json::Result<String> {
        Ok(serde_json::json!({
            "scale": self.scale,
        }).to_string())
    }
}

impl MqttEvent for super::IpAddrChangeEvent {
    fn topic(&self) -> String {
        "ip_address".into()
    }

    fn payload(&self) -> serde_json::Result<String> {
        Ok(serde_json::json!({
            "ip_address": self.addr.to_string(),
        }).to_string())
    }
}

impl MqttEvent for super::RebootEvent {
    fn topic(&self) -> String {
        String::from("system")
    }

    fn payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&BinarySensorPayload { state: self.state })
    }
}

impl MqttEvent for super::StationEvent {
    fn topic(&self) -> String {
        format!("station/{}", self.station_index)
    }

    fn payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&StationPayload {
            state: self.state,
            duration: self.duration.map(|dur| dur.num_seconds()),
            flow: self.flow_volume,
        })
    }
}

impl MqttEvent for super::RainDelayEvent {
    fn topic(&self) -> String {
        String::from("raindelay")
    }

    fn payload(&self) -> serde_json::Result<String> {
        serde_json::to_string(&RainDelayPayload { state: self.state })
    }
}