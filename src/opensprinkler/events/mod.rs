use serde::{Deserialize, Serialize};
use std::{
    net::IpAddr
};

use super::{config, station};

#[cfg(feature = "ifttt")]
pub mod ifttt;
#[cfg(feature = "ifttt")]
use ifttt::SendIftttWebhook;

#[cfg(feature = "mqtt")]
pub mod mqtt;
#[cfg(feature = "mqtt")]
use mqtt::PublishMqttMessage;

#[cfg(test)]
mod tests;

#[repr(u16)]
#[derive(Copy, Clone)]
pub enum EventType {
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EventsEnabled {
    pub program_start: bool,
    pub sensor1: bool,
    pub flow_sensor: bool,
    pub weather_update: bool,
    pub reboot: bool,
    pub station_off: bool,
    pub sensor2: bool,
    pub rain_delay: bool,
    pub station_on: bool,
}

impl EventsEnabled {
    pub fn is_enabled<E>(&self, event: &E) -> bool
    where
        E: Event,
    {
        match event.event_type() {
            EventType::ProgramStart => self.program_start,
            EventType::Sensor1 => self.sensor1,
            EventType::FlowSensor => self.flow_sensor,
            EventType::WeatherUpdate => self.weather_update,
            EventType::Reboot => self.reboot,
            EventType::StationOff => self.station_off,
            EventType::Sensor2 => self.sensor2,
            EventType::RainDelay => self.rain_delay,
            EventType::StationOn => self.station_on,
        }
    }
}

#[cfg(not(any(feature = "ifttt", feature = "mqtt")))]
pub trait Event: Send {
    fn event_type(&self) -> EventType;
}

#[cfg(all(feature = "ifttt", not(feature = "mqtt")))]
pub trait Event: Send + ifttt::WebHookEvent {
    fn event_type(&self) -> EventType;
}

#[cfg(all(feature = "mqtt", not(feature = "ifttt")))]
pub trait Event: Send + mqtt::MqttEvent {
    fn event_type(&self) -> EventType;
}

#[cfg(all(feature = "mqtt", feature = "ifttt"))]
pub trait Event: Send + mqtt::MqttEvent + ifttt::WebHookEvent {
    fn event_type(&self) -> EventType;
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

impl Event for ProgramStartEvent {
    fn event_type(&self) -> EventType {
        EventType::ProgramStart
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

impl Event for BinarySensorEvent {
    fn event_type(&self) -> EventType {
        match self.index {
            0 => EventType::Sensor1,
            1 => EventType::Sensor2,
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

impl Event for FlowSensorEvent {
    fn event_type(&self) -> EventType {
        EventType::FlowSensor
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

impl Event for WeatherUpdateEvent {
    fn event_type(&self) -> EventType {
        EventType::WeatherUpdate
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

impl Event for RebootEvent {
    fn event_type(&self) -> EventType {
        EventType::Reboot
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

impl Event for StationEvent {
    fn event_type(&self) -> EventType {
        if self.state {
            EventType::StationOn
        } else {
            EventType::StationOff
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

impl Event for RainDelayEvent {
    fn event_type(&self) -> EventType {
        EventType::RainDelay
    }
}

pub struct Events {
    #[cfg(feature = "mqtt")]
    pub mqtt_client: paho_mqtt::AsyncClient,
}

impl Events {
    pub fn new() -> result::Result<Self> {
        Ok(Self {
            #[cfg(feature = "mqtt")]
            // The default options use an empty client ID (therefore, the persistence type is None).
            mqtt_client: paho_mqtt::AsyncClient::new(paho_mqtt::CreateOptionsBuilder::new().finalize())?,
        })
    }

    pub fn setup(&mut self, config: &config::Config) {

        #[cfg(feature = "mqtt")]
        {
            let msg = paho_mqtt::MessageBuilder::new()
                .retained(true)
                .topic(format!("{}/{}", config.mqtt.root_topic, config.mqtt.availability_topic))
                .payload(config.mqtt.online_payload.as_bytes())
                .finalize();
            self.mqtt_client.set_connected_callback::<Box<paho_mqtt::ConnectedCallback>>(Box::new(move |client| {
                client.publish(msg.clone());
            }));
        }
    }

    pub fn push<E>(&self, config: &config::Config, event: E) -> result::Result<()>
    where
        E: Event,
    {
        #[cfg(feature = "ifttt")]
        if config.ifttt.events.is_enabled(&event) {
            self.ifttt_webhook(&config.ifttt, &event)?;
        }

        #[cfg(feature = "mqtt")]
        if config.mqtt.events.is_enabled(&event) {
            self.mqtt_publish(&config.mqtt, &event)?;
        }

        Ok(())
    }

    #[cfg(feature = "mqtt")]
    pub fn mqtt_connect_options(config: &mqtt::Config) -> paho_mqtt::connect_options::ConnectOptions {
        let will = paho_mqtt::MessageBuilder::new()
            .topic(format!("{}/{}", config.root_topic, config.availability_topic))
            .payload(config.offline_payload.as_bytes())
            .finalize();

        let mut builder = paho_mqtt::ConnectOptionsBuilder::new();
        builder.mqtt_version(config.version).clean_session(true).will_message(will);

        if let Some(uri) = config.uri() {
            builder.server_uris(&[uri]);
        }

        if let Some(ref username) = config.username {
            builder.user_name(username);
        }

        if let Some(ref password) = config.password {
            builder.password(password);
        }

        builder.finalize()
    }
}

pub mod result {
    use core::fmt;

    pub type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug)]
    pub enum Error {
        SerdeError(serde_json::Error),
        MqttError(paho_mqtt::Error),
        IftttRequestError(reqwest::Error),
    }

    impl std::error::Error for Error {}

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                Self::SerdeError(ref err) => write!(f, "Serde Error: {:?}", err),
                Self::MqttError(ref err) => write!(f, "MQTT Error: {:?}", err),
                Self::IftttRequestError(ref err) => write!(f, "IFTTT Webhook Error: {:?}", err),
            }
        }
    }

    impl From<serde_json::Error> for Error {
        fn from(err: serde_json::Error) -> Self {
            Self::SerdeError(err)
        }
    }

    impl From<paho_mqtt::Error> for Error {
        fn from(err: paho_mqtt::Error) -> Self {
            Self::MqttError(err)
        }
    }

    impl From<reqwest::Error> for Error {
        fn from(err: reqwest::Error) -> Self {
            Self::IftttRequestError(err)
        }
    }
}
