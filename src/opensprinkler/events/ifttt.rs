
use crate::{utils::duration_to_hms, opensprinkler::program};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Result;

#[derive(Clone, Serialize, Deserialize)]
pub struct EventConfig {
    /// IFTTT Webhooks URL
    pub web_hooks_url: String,

    /// IFTTT Webhooks API key
    pub web_hooks_key: Option<String>,

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

impl Default for EventConfig {
    fn default() -> Self {
        EventConfig {
            web_hooks_url: String::from("https://maker.ifttt.com"),
            web_hooks_key: None,
            program_start: false,
            sensor1: false,
            flow_sensor: false,
            weather_update: false,
            reboot: false,
            station_off: false,
            sensor2: false,
            rain_delay: false,
            station_on: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WebHookPayload {
    value1: String,
}

pub trait WebHookEvent {
    fn ifttt_event(&self) -> String;
    fn ifttt_payload(&self) -> String;
}

pub trait WebHookEventPayload: WebHookEvent {
    fn ifttt_url(&self, base: &str, key: &str) -> Url;
    fn ifttt_payload_json(&self) -> Result<String>;
}

impl<T> WebHookEventPayload for T
where
    T: WebHookEvent,
{
    fn ifttt_url(&self, base: &str, key: &str) -> reqwest::Url {
        let url = reqwest::Url::parse(format!("{}/trigger/{}/with/key/{}", base, self.ifttt_event(), key).as_str()).unwrap();

        url
    }

    fn ifttt_payload_json(&self) -> Result<String> {
        let payload = WebHookPayload { value1: self.ifttt_payload() };
        serde_json::to_string(&payload)
    }
}

impl WebHookEvent for super::ProgramStartEvent {
    fn ifttt_event(&self) -> String {
        String::from("program")
    }

    fn ifttt_payload(&self) -> String {
        let mut payload = String::new();

        // Program that was manually started
        if self.program_index == program::MANUAL_PROGRAM_ID {
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

impl WebHookEvent for super::BinarySensorEvent {
    fn ifttt_event(&self) -> String {
        format!("sensor{}", self.index)
    }

    fn ifttt_payload(&self) -> String {
        match self.state {
            false => format!("Sensor {} deactivated", self.index + 1),
            true => format!("Sensor {} activated", self.index + 1),
        }
    }
}

impl WebHookEvent for super::FlowSensorEvent {
    fn ifttt_event(&self) -> String {
        String::from("flow")
    }

    fn ifttt_payload(&self) -> String {
        format!("Flow count: {:.0}, volume: {:.2}", self.count, self.volume)
    }
}

impl WebHookEvent for super::WeatherUpdateEvent {
    fn ifttt_event(&self) -> String {
        String::from("weather")
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

impl WebHookEvent for super::RebootEvent {
    fn ifttt_event(&self) -> String {
        String::from("reboot")
    }

    fn ifttt_payload(&self) -> String {
        match self.state {
            false => String::from("Controller stopping."),
            true => String::from("Controller started."),
        }
    }
}

impl WebHookEvent for super::StationEvent {
    fn ifttt_event(&self) -> String {
        String::from("station")
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

impl WebHookEvent for super::RainDelayEvent {
    fn ifttt_event(&self) -> String {
        String::from("rain_delay")
    }

    fn ifttt_payload(&self) -> String {
        match self.state {
            false => String::from("Rain delay deactivated."),
            true => String::from("Rain delay activated."),
        }
    }
}
