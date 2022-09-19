use super::{config, program, station};
use std::any::Any;
use std::fs;
use std::io;
use std::net;

pub mod result;
pub mod ifttt;
pub mod mqtt;

pub mod legacy;
pub mod log;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

pub type Timestamp = chrono::DateTime<chrono::Utc>;

pub trait Event: legacy::LogEvent + ifttt::WebHookEvent + mqtt::MqttEvent + Any {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramStartEvent {
    pub program_index: Option<usize>,
    pub program_name: Option<String>,
    pub water_scale: Option<f32>,
}

impl ProgramStartEvent {
    pub fn new(program_index: usize, program_name: String, water_scale: f32) -> ProgramStartEvent {
        ProgramStartEvent {
            program_index: Some(program_index),
            program_name: Some(program_name),
            water_scale: Some(water_scale),
        }
    }
}

impl Event for ProgramStartEvent {}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinarySensorEvent {
    #[serde_as(as = "Option<serde_with::DurationSeconds<i64>>")]
    pub duration: Option<chrono::Duration>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub index: usize,
    pub state: bool,
}

impl BinarySensorEvent {
    pub fn new(index: usize, state: bool, timestamp_now: i64, timestamp_last: Option<i64>) -> Self {
        Self {
            duration: timestamp_last.map(|t| chrono::Duration::seconds(timestamp_now - t)),
            timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp_now, 0), chrono::Utc),
            index,
            state,
        }
    }

    pub fn duration(mut self, duration: chrono::Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

impl Event for BinarySensorEvent {}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSensorEvent {
    #[serde_as(as = "Option<serde_with::DurationSeconds<i64>>")]
    pub duration: Option<chrono::Duration>,
    pub end: chrono::DateTime<chrono::Utc>,
    pub count: i32,
    pub volume: f64,
}

impl FlowSensorEvent {
    pub fn new(count: i32, pulse_rate: u16) -> Self {
        Self {
            duration: None,
            end: chrono::Utc::now(),
            count,
            volume: f64::from(count * pulse_rate as i32),
        }
    }

    pub fn duration(&mut self, duration: chrono::Duration) -> &mut Self {
        self.duration = Some(duration);
        self
    }

    pub fn end(&mut self, end: chrono::DateTime<chrono::Utc>) -> &mut Self {
        self.end = end;
        self
    }
}

impl Event for FlowSensorEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterScaleChangeEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub scale: f32,
}

impl WaterScaleChangeEvent {
    pub fn new(scale: f32, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        Self { timestamp, scale }
    }
}

impl Event for WaterScaleChangeEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAddrChangeEvent {
    pub addr: net::IpAddr,
}

impl IpAddrChangeEvent {
    pub fn new(addr: net::IpAddr) -> Self {
        Self { addr }
    }
}

impl Event for IpAddrChangeEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebootEvent {
    pub state: bool,
}

impl RebootEvent {
    pub fn new(state: bool) -> Self {
        Self { state }
    }
}

impl Event for RebootEvent {}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationEvent {
    #[serde_as(as = "Option<serde_with::DurationSeconds<i64>>")]
    pub duration: Option<chrono::Duration>,
    pub state: bool,
    pub flow_volume: Option<f64>,
    pub station_index: station::StationIndex,
    pub station_name: String,
    pub program_type: Option<program::ProgramStartType>,
    pub program_index: Option<usize>,
    pub program_name: Option<String>,
    pub end_time: Option<i64>,
}

impl StationEvent {
    pub fn new(state: bool, station_index: station::StationIndex, station_name: &str) -> Self {
        Self {
            duration: None,
            state,
            flow_volume: None,
            station_index,
            station_name: station_name.into(),
            program_type: None,
            program_index: None,
            program_name: None,
            end_time: None,
        }
    }

    pub fn duration(mut self, duration: i64) -> Self {
        self.duration = Some(chrono::Duration::seconds(duration));
        self
    }

    pub fn flow_volume(mut self, flow_volume: Option<f64>) -> Self {
        self.flow_volume = flow_volume;
        self
    }

    pub fn program_type(mut self, program_type: program::ProgramStartType) -> Self {
        self.program_type = Some(program_type);
        self
    }

    pub fn program_index(mut self, program_index: Option<usize>) -> Self {
        self.program_index = program_index;
        self
    }

    pub fn program_name(mut self, program_name: String) -> Self {
        self.program_name = Some(program_name);
        self
    }

    pub fn end_time(mut self, end_time: i64) -> Self {
        self.end_time = Some(end_time);
        self
    }
}

impl Event for StationEvent {}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RainDelayEvent {
    #[serde_as(as = "Option<serde_with::DurationSeconds<i64>>")]
    pub duration: Option<chrono::Duration>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub state: bool,
}

impl RainDelayEvent {
    pub fn new(state: bool, timestamp_now: i64, timestamp_last: Option<i64>) -> Self {

        Self {
            duration: timestamp_last.map(|t| chrono::Duration::seconds(timestamp_now - t)),
            timestamp: chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(timestamp_now, 0), chrono::Utc),
            state,
        }
    }
}

impl Event for RainDelayEvent {}

pub struct Events {
    pub mqtt_client: paho_mqtt::AsyncClient,
}

impl Events {
    pub fn new() -> result::Result<Self> {
        Ok(Self {
            // The default options use an empty client ID (therefore, the persistence type is None).
            mqtt_client: paho_mqtt::AsyncClient::new(paho_mqtt::CreateOptionsBuilder::new().finalize())?,
        })
    }

    pub fn setup(&mut self, config: &config::Config) {
        let msg = paho_mqtt::MessageBuilder::new()
            .retained(true)
            .topic(format!("{}/{}", config.mqtt.root_topic, config.mqtt.availability_topic))
            .payload(config.mqtt.online_payload.as_bytes())
            .finalize();
        self.mqtt_client.set_connected_callback::<Box<paho_mqtt::ConnectedCallback>>(Box::new(move |client| {
            client.publish(msg.clone());
        }));
    }

    pub fn push(&self, config: &config::Config, event: &dyn Event) -> result::Result<()> {
        if self.is_ifttt_event_enabled(config, event) {
            self.ifttt_webhook(&config.ifttt, event)?;
        }

        self.mqtt_publish(&config.mqtt, event)?;

        self.log_write(&config.event_log, event)?;

        Ok(())
    }

    fn is_ifttt_event_enabled(&self, _config: &config::Config, _event: &dyn Event) -> bool {
        /* return if let Some(_) = (event as &dyn Any).downcast_ref::<ProgramStartEvent>() {
            config.ifttt.events.program_start
        } else if let Some(event) = (event as &dyn Any).downcast_ref::<BinarySensorEvent>() {
            (config.ifttt.events.sensor1 && event.index == 0) || (config.ifttt.events.sensor2 && event.index == 1)
        } else if let Some(_) = (event as &dyn Any).downcast_ref::<FlowSensorEvent>() {
            config.ifttt.events.flow_sensor
        } else if let Some(_) = (event as &dyn Any).downcast_ref::<WaterScaleChangeEvent>() {
            config.ifttt.events.weather_update
        } else if let Some(_) = (event as &dyn Any).downcast_ref::<IpAddrChangeEvent>() {
            config.ifttt.events.weather_update
        } else if let Some(_) = (event as &dyn Any).downcast_ref::<RebootEvent>() {
            config.ifttt.events.reboot
        } else if let Some(event) = (event as &dyn Any).downcast_ref::<StationEvent>() {
            config.ifttt.events.station_off && event.state == false
        } else if let Some(_) = (event as &dyn Any).downcast_ref::<RainDelayEvent>() {
            config.ifttt.events.rain_delay
        } else {
            false
        } */
        false
    }

    fn log_write(&self, config: &log::Config, event: &dyn Event) -> result::Result<()> {
        let mut path = config.path.clone();

        // Create dir if necessary
        if !path.exists() {
            fs::create_dir_all(&path)?;
        } else {
            if !path.is_dir() {
                return Err(result::Error::IoError(io::Error::new(io::ErrorKind::AlreadyExists, "Log path is not a directory.")));
            }
        }

        // Filename
        path.push(chrono::Utc::now().format("%Y%m%d").to_string());
        path.set_extension("bin");

        let file = fs::OpenOptions::new().read(true).write(true).create(true).open(path)?;

        //todo!();

        /* let encoded = rmp_serde::encode::to_vec_named(&data).unwrap();
        log::append(file, encoded.as_slice()) */
        Ok(())
    }

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

    fn mqtt_publish(&self, config: &mqtt::Config, event: &dyn Event) -> Result<(), serde_json::Error> {
        self.mqtt_client
            .publish(paho_mqtt::MessageBuilder::new().topic(format!("{}/{}", config.root_topic, event.topic())).payload(mqtt::MqttEvent::payload(event)?).finalize());

        Ok(())
    }

    fn ifttt_webhook(&self, config: &ifttt::Config, event: &dyn Event) -> result::Result<()> {
        use super::http;

        let body = serde_json::json!({
            "value1": ifttt::WebHookEvent::payload(event),
        })
        .to_string();

        if let Ok(url) = reqwest::Url::parse(format!("{}/trigger/{}/with/key/{}", config.web_hooks_url, event.ifttt_event(), config.web_hooks_key).as_str()) {
            let response = http::request::build_client()
                .unwrap()
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_static("application/json"))
                .body(body)
                .send();

            if let Err(err) = response {
                tracing::error!("Error making IFTTT Web Hook request: {:?}", err);
                return Err(result::Error::IftttRequestError(err));
            }
        }

        Ok(())
    }
}
