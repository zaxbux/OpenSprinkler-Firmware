use core::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use serde_json::Result;
extern crate paho_mqtt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MQTTConfig {
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
}

impl MQTTConfig {
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

impl Default for MQTTConfig {
    fn default() -> Self {
        MQTTConfig {
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

        }
    }
}

impl fmt::Display for MQTTConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.uri())
    }
}

/// Program Start
#[derive(Serialize, Deserialize)]
pub struct ProgramSchedPayload {
    pub program_id: usize,
    pub program_name: String,
    pub manual: bool,
    pub water_level: u8,
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
    pub count: u64,
    pub volume: f64,
}

/// Weather Update
#[derive(Serialize, Deserialize)]
pub struct WeatherUpdatePayload {
    pub scale: Option<u8>,
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

pub trait Payload<P>
where
    P: serde::Serialize,
{
    fn mqtt_topic(&self) -> String;
    fn mqtt_payload(&self) -> P;
}

pub trait EventPayload<P>: Payload<P>
where
    P: serde::Serialize,
{
    fn mqtt_payload_json(&self) -> Result<String>;
}

impl<E, P> EventPayload<P> for E
where
    E: Payload<P> + super::EventType,
    P: serde::Serialize,
{
    #[inline]
    fn mqtt_payload_json(&self) -> Result<String> {
        serde_json::to_string(&(self.mqtt_payload()))
    }
}

impl Payload<ProgramSchedPayload> for super::ProgramStartEvent {
    fn mqtt_topic(&self) -> String {
        String::from("program")
    }

    fn mqtt_payload(&self) -> ProgramSchedPayload {
        ProgramSchedPayload {
            program_id: self.program_id,
            program_name: self.program_name.clone(),
            manual: self.manual,
            water_level: self.water_level,
        }
    }
}

impl Payload<BinarySensorPayload> for super::BinarySensorEvent {
    fn mqtt_topic(&self) -> String {
        format!("sensor{}", self.index)
    }

    fn mqtt_payload(&self) -> BinarySensorPayload {
        BinarySensorPayload { state: self.state }
    }
}

impl Payload<FlowSensorPayload> for super::FlowSensorEvent {
    fn mqtt_topic(&self) -> String {
        String::from("sensor/flow")
    }

    fn mqtt_payload(&self) -> FlowSensorPayload {
        FlowSensorPayload { count: self.count, volume: self.volume }
    }
}

impl Payload<WeatherUpdatePayload> for super::WeatherUpdateEvent {
    fn mqtt_topic(&self) -> String {
        String::from("weather")
    }

    fn mqtt_payload(&self) -> WeatherUpdatePayload {
        WeatherUpdatePayload {
            scale: self.scale,
            external_ip: self.external_ip,
        }
    }
}

impl Payload<BinarySensorPayload> for super::RebootEvent {
    fn mqtt_topic(&self) -> String {
        String::from("system")
    }

    fn mqtt_payload(&self) -> BinarySensorPayload {
        BinarySensorPayload { state: self.state }
    }
}

impl Payload<StationPayload> for super::StationEvent {
    fn mqtt_topic(&self) -> String {
        format!("station/{}", self.station_id)
    }

    fn mqtt_payload(&self) -> StationPayload {
        StationPayload {
            state: self.state,
            duration: self.duration,
            flow: self.flow,
        }
    }
}

impl Payload<RainDelayPayload> for super::RainDelayEvent {
    fn mqtt_topic(&self) -> String {
        String::from("raindelay")
    }

    fn mqtt_payload(&self) -> RainDelayPayload {
        RainDelayPayload { state: self.state }
    }
}
