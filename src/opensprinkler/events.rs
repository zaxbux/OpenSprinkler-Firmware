use std::net::IpAddr;
use reqwest::header;

use super::OpenSprinkler;

pub mod ifttt;

#[cfg(feature = "mqtt")]
pub mod mqtt;

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

// region: Program Scheduled Run
pub struct ProgramSchedEvent {
    pub program_id: usize,
    pub program_name: String,
    pub manual: bool,
    pub water_level: u8,
}

impl ProgramSchedEvent {
    pub fn new(program_id: usize, program_name: String, manual: bool, water_level: u8) -> ProgramSchedEvent {
        ProgramSchedEvent {
            program_id,
            program_name,
            manual,
            water_level,
        }
    }
}

impl EventType for ProgramSchedEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::ProgramSched
    }
}
// endregion

// region: Sensor
pub struct BinarySensorEvent {
    index: usize,
    pub state: bool,
}

impl BinarySensorEvent {
    pub fn new(index: usize, state: bool) -> BinarySensorEvent {
        BinarySensorEvent { index, state }
    }
}

impl EventType for BinarySensorEvent {
    fn event_type(&self) -> NotifyEvent {
        match self.index {
            0 => NotifyEvent::Sensor1,
            1 => NotifyEvent::Sensor2,
            _ => unimplemented!(),
        }
    }
}
// endregion

// region: Flow Sensor
pub struct FlowSensorEvent {
    pub count: u32,
    pub volume: f32,
}

impl FlowSensorEvent {
    pub fn new(count: u32, pulse_rate: u16) -> FlowSensorEvent {
        FlowSensorEvent {
            count,
            volume: count as f32 * f32::from(pulse_rate),
        }
    }
}

impl EventType for FlowSensorEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::FlowSensor
    }
}
// endregion

// region: Weather Update
pub struct WeatherUpdateEvent {
    pub scale: Option<u8>,
    pub external_ip: Option<IpAddr>,
}

impl WeatherUpdateEvent {
    pub fn new(scale: Option<u8>, external_ip: Option<IpAddr>) -> WeatherUpdateEvent {
        WeatherUpdateEvent { scale, external_ip }
    }
}

impl EventType for WeatherUpdateEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::WeatherUpdate
    }
}
// endregion

// region: Reboot
pub struct RebootEvent {
    pub state: bool,
}

impl RebootEvent {
    pub fn new(state: bool) -> RebootEvent {
        RebootEvent { state }
    }
}

impl EventType for RebootEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::Reboot
    }
}
// endregion

// region: Station
pub struct StationEvent {
    pub station_id: usize,
    pub station_name: String,
    pub state: bool,
    pub duration: Option<u32>,
    pub flow: Option<f32>,
}

impl StationEvent {
    pub fn new(station_id: usize, station_name: String, state: bool, duration: Option<u32>, flow: Option<f32>) -> StationEvent {
        StationEvent {
            station_id,
            station_name,
            state,
            duration,
            flow,
        }
    }
}

impl EventType for StationEvent {
    fn event_type(&self) -> NotifyEvent {
        if self.state {
            NotifyEvent::StationOn
        } else {
            NotifyEvent::StationOff
        }
    }
}
// endregion

// region: Rain Delay
pub struct RainDelayEvent {
    pub state: bool,
}

impl RainDelayEvent {
    pub fn new(state: bool) -> RainDelayEvent {
        RainDelayEvent { state }
    }
}

impl EventType for RainDelayEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::RainDelay
    }
}
// endregion

pub trait EventType {
    fn event_type(&self) -> NotifyEvent;
}

#[cfg(feature = "mqtt")]
pub trait Event<S>: EventType + ifttt::WebHookEventPayload + mqtt::EventPayload<S>
where
    S: serde::Serialize,
{
}

#[cfg(feature = "mqtt")]
impl<S, T> Event<S> for T
where
    T: EventType + ifttt::WebHookEvent + mqtt::Payload<S>,
    S: serde::Serialize,
{}

#[cfg(not(feature = "mqtt"))]
pub trait Event: EventType + ifttt::WebHookEventPayload {}

#[cfg(not(feature = "mqtt"))]
impl<T> Event for T
where
    T: EventType + ifttt::WebHookEvent,
{}


/// Emits IFTTT and MQTT events (if enabled)
#[cfg(feature = "mqtt")]
pub fn push_message<E, S>(open_sprinkler: &OpenSprinkler, event: &E)
where
    E: Event<S>,
    S: serde::Serialize,
{
    if open_sprinkler.is_mqtt_enabled() {
        open_sprinkler.mqtt.publish(event);
    }

    if ifttt_event_enabled(open_sprinkler, event) {
        //ifttt_webhook(event.ifttt_payload(), open_sprinkler.sopts.ifkey.as_str());
        ifttt_webhook(event, open_sprinkler.controller_config.sopts.ifkey.as_str());
    }
}

#[cfg(not(feature = "mqtt"))]
pub fn push_message<E>(open_sprinkler: &OpenSprinkler, event: &E)
where
    E: Event,
{
    if open_sprinkler.is_mqtt_enabled() {
        tracing::warn!("MQTT is enabled on the controller but the feature is not compiled")
    }

    if ifttt_event_enabled(open_sprinkler, event) {
        //ifttt_webhook(event.ifttt_payload(), open_sprinkler.sopts.ifkey.as_str());
        ifttt_webhook(event, open_sprinkler.controller_config.sopts.ifkey.as_str());
    }
}

fn ifttt_event_enabled(open_sprinkler: &OpenSprinkler, event: &dyn EventType) -> bool {
    //let ifttt_enabled: bool = (open_sprinkler.iopts.ife & (event.event_type() as u8)) == 1;
    // @todo This can be more efficient:
    match event.event_type() {
        NotifyEvent::ProgramSched => open_sprinkler.controller_config.iopts.ifttt_events.program_sched,
        NotifyEvent::Sensor1 => open_sprinkler.controller_config.iopts.ifttt_events.sensor1,
        NotifyEvent::FlowSensor => open_sprinkler.controller_config.iopts.ifttt_events.flow_sensor,
        NotifyEvent::WeatherUpdate => open_sprinkler.controller_config.iopts.ifttt_events.weather_update,
        NotifyEvent::Reboot => open_sprinkler.controller_config.iopts.ifttt_events.reboot,
        NotifyEvent::StationOff => open_sprinkler.controller_config.iopts.ifttt_events.station_off,
        NotifyEvent::Sensor2 => open_sprinkler.controller_config.iopts.ifttt_events.sensor2,
        NotifyEvent::RainDelay => open_sprinkler.controller_config.iopts.ifttt_events.rain_delay,
        NotifyEvent::StationOn => open_sprinkler.controller_config.iopts.ifttt_events.station_on,
    }
}

fn ifttt_webhook(event: &dyn ifttt::WebHookEventPayload, key: &str)
{
    // @todo log request failures
    let mut url = reqwest::Url::parse(ifttt::WEBHOOK_URL).expect("");
    let _ = url.path_segments_mut().and_then(|mut path| {
        path.push("/trigger/sprinkler/with/key/").push(key);
        Ok(())
    });

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .header(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"))
        .body(event.ifttt_payload_json().expect("Error getting IFTTT payload"))
        .send();

    if let Err(error) = response {
        tracing::error!("Error making IFTTT Web Hook request: {:?}", error);
    }
}
