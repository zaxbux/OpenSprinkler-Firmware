
use crate::utils::duration_to_hms;
use serde::{Deserialize, Serialize};
use serde_json::Result;

/// @todo Make configurable
pub const WEBHOOK_URL: &'static str = "https://maker.ifttt.com";

#[derive(Serialize, Deserialize)]
pub struct WebHookPayload {
    value1: String,
}

pub trait WebHookEvent {
    fn ifttt_payload(&self) -> String;
}

pub trait WebHookEventPayload: WebHookEvent {
    fn ifttt_payload_json(&self) -> Result<String>;
}

impl<T> WebHookEventPayload for T
where
    T: WebHookEvent,
{
    #[inline]
    fn ifttt_payload_json(&self) -> Result<String> {
        let payload = WebHookPayload { value1: self.ifttt_payload() };
        serde_json::to_string(&payload)
    }
}

impl WebHookEvent for super::ProgramSchedEvent {
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

impl WebHookEvent for super::BinarySensorEvent {
    fn ifttt_payload(&self) -> String {
        let mut payload = String::from(format!("Sensor {}", self.index));

        if self.state {
            payload.push_str(" activated.");
        } else {
            payload.push_str(" deactivated.");
        }

        payload
    }
}

impl WebHookEvent for super::FlowSensorEvent {
    fn ifttt_payload(&self) -> String {
        format!("Flow count: {:.0}, volume: {:.2}", self.count, self.volume)
    }
}

impl WebHookEvent for super::WeatherUpdateEvent {
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

impl WebHookEvent for super::StationEvent {
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
