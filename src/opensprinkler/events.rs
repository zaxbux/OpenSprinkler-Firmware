use std::net::IpAddr;

use reqwest::header::{HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Result;

use crate::utils::duration_to_hms;

use super::OpenSprinkler;

pub const IFTTT_WEBHOOK_URL: &'static str = "https://maker.ifttt.com";

mod mqtt_json {
    use std::net::IpAddr;

    use serde::{Deserialize, Serialize};

    /// Program Start
    #[derive(Serialize, Deserialize)]
    pub struct ProgramSched {
        pub program_id: usize,
        pub program_name: String,
        pub manual: bool,
        pub water_level: u8,
    }

    /// Sensor
    #[derive(Serialize, Deserialize)]
    pub struct Sensor {
        pub state: bool,
    }

    /// Rain Delay state
    #[derive(Serialize, Deserialize)]
    pub struct RainDelay {
        pub state: bool,
    }

    #[derive(Serialize, Deserialize)]
    pub struct FlowSensor {
        pub count: u32,
        pub volume: f32,
    }

    /// Weather Update
    #[derive(Serialize, Deserialize)]
    pub struct WeatherUpdate {
        pub scale: Option<u8>,
        pub external_ip: Option<IpAddr>,
    }

    /// Controller reboot
    #[derive(Serialize, Deserialize)]
    pub struct Reboot {
        pub state: String,
    }

    /// Station On/Off
    #[derive(Serialize, Deserialize)]
    pub struct Station {
        pub state: bool,
        pub duration: Option<u32>,
        pub flow: Option<f32>,
    }
}

#[derive(Serialize, Deserialize)]
pub struct IFTTTWebHookPayload {
    value1: String,
}

#[repr(u16)]
#[derive(Copy, Clone)]
pub enum NotifyEvent {
    ProgramSched = 0x0001,
    Sensor1 = 0x0002,
    FlowSensor = 0x0004,
    WeatherUpdate = 0x0008,
    Reboot = 0x0010,
    StationOff = 0x0020,
    Sensor2 = 0x0040,
    RainDelay = 0x0080,
    StationOn = 0x0100,
}

pub trait Event<P>
where
    P: serde::Serialize,
{
    const MQTT_TOPIC: &'static str;

    fn event_type(&self) -> NotifyEvent;

    fn mqtt_payload(&self) -> P;
    fn ifttt_payload(&self) -> String;
}

trait IFTTTWebHookEvent<P> {
    fn ifttt_payload_json(&self) -> Result<String>;
}

impl<T, P> IFTTTWebHookEvent<P> for T
where
    T: Event<P>,
    P: serde::Serialize,
{
    #[inline]
    fn ifttt_payload_json(&self) -> Result<String> {
        let payload = IFTTTWebHookPayload {
            value1: self.ifttt_payload(),
        };
        serde_json::to_string(&payload)
    }
}

trait MQTTPayload<P> {
    fn mqtt_payload_json(&self, payload: &P) -> Result<String>;
}

impl<P, T> MQTTPayload<P> for T
where
    T: Event<P>,
    P: serde::Serialize,
{
    #[inline]
    fn mqtt_payload_json(&self, payload: &P) -> Result<String> {
        serde_json::to_string(&(self.mqtt_payload()))
    }
}

// region: Program Scheduled Run
pub struct ProgramSched {
    pub program_id: usize,
    pub program_name: String,
    pub manual: bool,
    pub water_level: u8,
}

impl ProgramSched {
    pub fn new(
        program_id: usize,
        program_name: String,
        manual: bool,
        water_level: u8,
    ) -> ProgramSched {
        ProgramSched {
            program_id,
            program_name,
            manual,
            water_level,
        }
    }
}

impl Event<mqtt_json::ProgramSched> for ProgramSched {
    const MQTT_TOPIC: &'static str = "opensprinkler/program";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::ProgramSched
    }

    fn mqtt_payload(&self) -> mqtt_json::ProgramSched {
        mqtt_json::ProgramSched {
            program_id: self.program_id,
            program_name: self.program_name.clone(),
            manual: self.manual,
            water_level: self.water_level,
        }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();

        // Program that was manually started
        if self.program_id == 254 {
            payload.push_str("Manually started ");
        } else {
            payload.push_str("Automatically scheduled ");
        }
        payload.push_str("Program ");
        payload.push_str(&self.program_name);
        payload.push_str(format!(" with {}% water level.", self.water_level).as_str());

        payload
    }
}
// endregion

// region: Sensor 1
pub struct Sensor1 {
    pub state: bool,
}

impl Sensor1 {
    pub fn new(state: bool) -> Sensor1 {
        Sensor1 { state }
    }
}

impl Event<mqtt_json::Sensor> for Sensor1 {
    const MQTT_TOPIC: &'static str = "opensprinkler/sensor1";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::Sensor1
    }

    fn mqtt_payload(&self) -> mqtt_json::Sensor {
        mqtt_json::Sensor { state: self.state }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();
        payload.push_str("Sensor 1 ");

        if self.state {
            payload.push_str("activated.");
        } else {
            payload.push_str("deactivated.");
        }

        payload
    }
}
// endregion

// region: Flow Sensor
pub struct FlowSensor {
    pub count: u32,
    pub volume: f32,
}

impl FlowSensor {
    pub fn new(count: u32, pulse_rate: u16) -> FlowSensor {
        FlowSensor {
            count,
            volume: count as f32 * f32::from(pulse_rate),
        }
    }
}

impl Event<mqtt_json::FlowSensor> for FlowSensor {
    const MQTT_TOPIC: &'static str = "opensprinkler/sensor/flow";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::FlowSensor
    }

    fn mqtt_payload(&self) -> mqtt_json::FlowSensor {
        mqtt_json::FlowSensor {
            count: self.count,
            volume: self.volume,
        }
    }

    fn ifttt_payload(&self) -> String {
        format!("Flow count: {:.0}, volume: {:.2}", self.count, self.volume)
    }
}
// endregion

// region: Weather Update
pub struct WeatherUpdate {
    pub scale: Option<u8>,
    pub external_ip: Option<IpAddr>,
}

impl WeatherUpdate {
    pub fn new(scale: Option<u8>, external_ip: Option<IpAddr>) -> WeatherUpdate {
        WeatherUpdate { scale, external_ip }
    }
}

impl Event<mqtt_json::WeatherUpdate> for WeatherUpdate {
    const MQTT_TOPIC: &'static str = "opensprinkler/weather";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::WeatherUpdate
    }

    fn mqtt_payload(&self) -> mqtt_json::WeatherUpdate {
        mqtt_json::WeatherUpdate {
            scale: self.scale,
            external_ip: self.external_ip,
        }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();

        if self.external_ip.is_some() {
            payload.push_str("External IP updated: {} ");
            payload.push_str(self.external_ip.unwrap().to_string().as_str());
        }

        if self.scale.is_some() {
            payload.push_str(format!("Water level updated: {}%", self.scale.unwrap()).as_str());
        }

        payload
    }
}
// endregion

// region: Reboot
pub struct Reboot {
    pub state: bool,
}

impl Reboot {
    pub fn new(state: bool) -> Reboot {
        Reboot { state }
    }
}

impl Event<mqtt_json::Sensor> for Reboot {
    const MQTT_TOPIC: &'static str = "opensprinkler/system";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::Reboot
    }

    fn mqtt_payload(&self) -> mqtt_json::Sensor {
        mqtt_json::Sensor { state: self.state }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();
        payload.push_str("Controller ");

        if self.state {
            payload.push_str("process started.");
        } else {
            payload.push_str("shutting down.");
        }

        payload
    }
}
// endregion

// region: Station
pub struct Station {
    pub station_id: usize,
    pub station_name: String,
    pub state: bool,
    pub duration: Option<u32>,
    pub flow: Option<f32>,
}

impl Station {
    pub fn new(
        station_id: usize,
        station_name: String,
        state: bool,
        duration: Option<u32>,
        flow: Option<f32>,
    ) -> Station {
        Station {
            station_id,
            station_name,
            state,
            duration,
            flow,
        }
    }
}

impl Event<mqtt_json::Station> for Station {
    const MQTT_TOPIC: &'static str = "opensprinkler/station/{}"; // @todo format with station ID

    fn event_type(&self) -> NotifyEvent {
        if self.state {
            NotifyEvent::StationOn
        } else {
            NotifyEvent::StationOff
        }
    }

    fn mqtt_payload(&self) -> mqtt_json::Station {
        mqtt_json::Station {
            state: self.state,
            duration: self.duration,
            flow: self.flow,
        }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();
        payload.push_str(format!("Station {} ", self.station_name).as_str());
        payload.push_str(if self.state { "opened. " } else { "closed. " });

        if self.state == false && self.duration.is_some() {
            payload.push_str("It ran for ");
            payload.push_str(duration_to_hms(self.duration.unwrap()).as_str());
        }

        if self.flow.is_some() {
            payload.push_str(format!("Flow rate: {:.2}", self.flow.unwrap()).as_str());
        }

        payload
    }
}
// endregion

// region: Sensor 2
pub struct Sensor2 {
    pub state: bool,
}

impl Sensor2 {
    pub fn new(state: bool) -> Sensor2 {
        Sensor2 { state }
    }
}

impl Event<mqtt_json::Sensor> for Sensor2 {
    const MQTT_TOPIC: &'static str = "opensprinkler/sensor2";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::Sensor2
    }

    fn mqtt_payload(&self) -> mqtt_json::Sensor {
        mqtt_json::Sensor { state: self.state }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();
        payload.push_str("Sensor 2 ");

        if self.state {
            payload.push_str("activated.");
        } else {
            payload.push_str("deactivated.");
        }

        payload
    }
}
// endregion

// region: Rain Delay
pub struct RainDelay {
    pub state: bool,
}

impl RainDelay {
    pub fn new(state: bool) -> RainDelay {
        RainDelay { state }
    }
}

impl Event<mqtt_json::RainDelay> for RainDelay {
    const MQTT_TOPIC: &'static str = "opensprinkler/raindelay";

    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::RainDelay
    }

    fn mqtt_payload(&self) -> mqtt_json::RainDelay {
        mqtt_json::RainDelay { state: self.state }
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();
        payload.push_str("Rain delay ");

        if self.state {
            payload.push_str("activated.");
        } else {
            payload.push_str("deactivated.");
        }

        payload
    }
}
// endregion

pub fn push_message<E, P>(open_sprinkler: &OpenSprinkler, event: E)
where
    E: Event<P>,
    P: serde::Serialize,
{
    let ifttt_enabled: bool = (open_sprinkler.iopts.ife & (event.event_type() as u8)) == 1;

    if !ifttt_enabled
    /* && !open_sprinkler.mqtt.enabled()*/
    {
        return;
    }

    /*     if (open_sprinkler.mqtt.enabled()) {
        open_sprinkler.mqtt.publish(event);
    } */

    if ifttt_enabled {
        // @todo log request failures
        let client = reqwest::blocking::Client::new();
        let _ = client
            .post(format!(
                "{}/trigger/sprinkler/with/key/{}",
                IFTTT_WEBHOOK_URL,
                open_sprinkler.sopts.ifkey
            ))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .body(event.ifttt_payload())
            .send()
            .expect("Error making HTTP station request");
    }
}
