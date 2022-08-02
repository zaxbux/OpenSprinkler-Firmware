use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use serde_json::Result;

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
    pub count: u32,
    pub volume: f32,
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
    pub duration: Option<u32>,
    pub flow: Option<f32>,
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

impl Payload<ProgramSchedPayload> for super::ProgramSchedEvent {
    fn mqtt_topic(&self) -> String {
        String::from("opensprinkler/program")
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
        format!("opensprinkler/sensor{}", self.index)
    }

    fn mqtt_payload(&self) -> BinarySensorPayload {
        BinarySensorPayload { state: self.state }
    }
}

impl Payload<FlowSensorPayload> for super::FlowSensorEvent {
    fn mqtt_topic(&self) -> String {
        String::from("opensprinkler/sensor/flow")
    }

    fn mqtt_payload(&self) -> FlowSensorPayload {
        FlowSensorPayload { count: self.count, volume: self.volume }
    }
}

impl Payload<WeatherUpdatePayload> for super::WeatherUpdateEvent {
    fn mqtt_topic(&self) -> String {
        String::from("opensprinkler/weather")
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
        String::from("opensprinkler/system")
    }

    fn mqtt_payload(&self) -> BinarySensorPayload {
        BinarySensorPayload { state: self.state }
    }
}

impl Payload<StationPayload> for super::StationEvent {
    fn mqtt_topic(&self) -> String {
        format!("opensprinkler/station/{}", self.station_id)
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
        String::from("opensprinkler/raindelay")
    }

    fn mqtt_payload(&self) -> RainDelayPayload {
        RainDelayPayload { state: self.state }
    }
}
