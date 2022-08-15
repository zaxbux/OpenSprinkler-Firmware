use serde::{Deserialize, Serialize};

use crate::{
    opensprinkler::{http, program},
    utils,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// IFTTT Webhooks URL
    pub web_hooks_url: String,

    /// IFTTT Webhooks API key
    pub web_hooks_key: String,

    /// Enabled events
    pub events: super::EventsEnabled,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            web_hooks_url: String::from("https://maker.ifttt.com"),
            web_hooks_key: String::from(""),
            events: super::EventsEnabled::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WebHookPayload {
    value1: String,
}

impl WebHookPayload {
    pub fn new(value1: String) -> Self {
        Self { value1 }
    }
}

pub trait WebHookEvent {
    fn ifttt_event(&self) -> String;
    fn ifttt_payload(&self) -> String;
}

impl WebHookEvent for super::ProgramStartEvent {
    fn ifttt_event(&self) -> String {
        "program".into()
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
        payload.push_str(format!(" with {}% water level.", self.water_scale).as_str());

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
            payload.push_str(utils::duration_to_hms(self.duration.unwrap()).as_str());
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

pub(super) trait SendIftttWebhook {
    fn ifttt_webhook<E: super::Event>(&self, config: &Config, event: &E) -> super::result::Result<()>;
}

impl SendIftttWebhook for super::Events {
    fn ifttt_webhook<E>(&self, config: &Config, event: &E) -> super::result::Result<()>
    where
        E: super::Event,
    {
        let body = serde_json::json!({
            "value1": event.ifttt_payload(),
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
                return Err(super::result::Error::IftttRequestError(err));
            }
        }

        Ok(())
    }
}
