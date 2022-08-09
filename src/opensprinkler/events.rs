use std::net::IpAddr;
use reqwest::header;

use super::{OpenSprinkler, http::request, station};

pub mod ifttt;

#[cfg(feature = "mqtt")]
pub mod mqtt;

#[cfg(test)]
mod tests;

#[repr(u16)]
#[derive(Copy, Clone)]
pub enum NotifyEvent {
    ProgramStart = 0x0001,
    Sensor1 = 0x0002,
    FlowSensor = 0x0004,
    WeatherUpdate = 0x0008,
    Reboot = 0x0010,
    StationOff = 0x0020,
    Sensor2 = 0x0040,
    RainDelay = 0x0080,
    StationOn = 0x0100,
}

pub struct ProgramStartEvent {
    pub program_index: usize,
    pub program_name: String,
    /* pub manual: bool, */
    pub water_scale: f32,
}

impl ProgramStartEvent {
    pub fn new(program_index: usize, program_name: String, /*manual: bool,*/ water_scale: f32) -> ProgramStartEvent {
        ProgramStartEvent {
            program_index,
            program_name,
            /*manual,*/
            water_scale,
        }
    }
}

impl EventType for ProgramStartEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::ProgramStart
    }
}

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

pub struct FlowSensorEvent {
    pub count: i64,
    pub volume: f64,
}

impl FlowSensorEvent {
    pub fn new(count: i64, pulse_rate: u16) -> FlowSensorEvent {
        FlowSensorEvent {
            count,
            volume: count as f64 * f64::from(pulse_rate),
        }
    }
}

impl EventType for FlowSensorEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::FlowSensor
    }
}

pub struct WeatherUpdateEvent {
    pub scale: Option<f32>,
    pub external_ip: Option<IpAddr>,
}

impl WeatherUpdateEvent {
    pub fn new(scale: Option<f32>, external_ip: Option<IpAddr>) -> WeatherUpdateEvent {
        WeatherUpdateEvent { scale, external_ip }
    }

    pub fn water_scale(scale: f32) -> Self {
        WeatherUpdateEvent { scale: Some(scale), external_ip: None }
    }

    pub fn external_ip(external_ip: IpAddr) -> Self {
        WeatherUpdateEvent { scale: None, external_ip: Some(external_ip) }
    }
}

impl EventType for WeatherUpdateEvent {
    fn event_type(&self) -> NotifyEvent {
        NotifyEvent::WeatherUpdate
    }
}

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

pub struct StationEvent {
    pub station_index: station::StationIndex,
    pub station_name: String,
    pub state: bool,
    pub duration: Option<i64>,
    pub flow: Option<f64>,
}

impl StationEvent {
    pub fn new(station_index: station::StationIndex, station_name: String, state: bool, duration: Option<i64>, flow: Option<f64>) -> StationEvent {
        StationEvent {
            station_index,
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
        let _ = open_sprinkler.mqtt.publish(event);
    }

    if ifttt_event_enabled(open_sprinkler, event) {
        if let Some(ref ifttt_api_key) = open_sprinkler.config.ifttt.web_hooks_key {
            ifttt_webhook(event, &open_sprinkler.config.ifttt.web_hooks_url, ifttt_api_key);
        } else {
            tracing::error!("IFTTT Web Hook API key unset");
        }
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
        if let Some(ref ifttt_api_key) = open_sprinkler.config.ifttt.web_hooks_key {
            ifttt_webhook(event, &open_sprinkler.config.ifttt.web_hooks_url, ifttt_api_key);
        } else {
            tracing::error!("IFTTT Web Hook API key unset");
        }
    }
}

fn ifttt_event_enabled(open_sprinkler: &OpenSprinkler, event: &dyn EventType) -> bool {
    // @todo This can be more efficient:
    match event.event_type() {
        NotifyEvent::ProgramStart => open_sprinkler.config.ifttt.program_start,
        NotifyEvent::Sensor1 => open_sprinkler.config.ifttt.sensor1,
        NotifyEvent::FlowSensor => open_sprinkler.config.ifttt.flow_sensor,
        NotifyEvent::WeatherUpdate => open_sprinkler.config.ifttt.weather_update,
        NotifyEvent::Reboot => open_sprinkler.config.ifttt.reboot,
        NotifyEvent::StationOff => open_sprinkler.config.ifttt.station_off,
        NotifyEvent::Sensor2 => open_sprinkler.config.ifttt.sensor2,
        NotifyEvent::RainDelay => open_sprinkler.config.ifttt.rain_delay,
        NotifyEvent::StationOn => open_sprinkler.config.ifttt.station_on,
    }
}

fn ifttt_webhook(event: &dyn ifttt::WebHookEventPayload, base: &str, key: &str)
{
    // @todo log request failures
    let body = event.ifttt_payload_json();

    if let Ok(body) = body {
        if let Ok(url) = event.ifttt_url(base, key) {
            let response = request::build_client().unwrap()
                .post(url)
                .header(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"))
                .body(body)
                .send();

            if let Err(error) = response {
                tracing::error!("Error making IFTTT Web Hook request: {:?}", error);
            }
        }
    }
}
