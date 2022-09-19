use serde::{Deserialize, Serialize};

use crate::{
    opensprinkler::{program, config},
    utils,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// IFTTT Webhooks URL
    pub web_hooks_url: String,

    /// IFTTT Webhooks API key
    pub web_hooks_key: String,

    /// Enabled events
    pub events: config::EventsEnabled,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            web_hooks_url: String::from("https://maker.ifttt.com"),
            web_hooks_key: String::from(""),
            events: config::EventsEnabled::default(),
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
    fn payload(&self) -> String;

}

impl WebHookEvent for super::ProgramStartEvent {
    fn ifttt_event(&self) -> String {
        "program".into()
    }

    fn payload(&self) -> String {
        let mut payload = String::new();

        // Program that was manually started
        if self.program_index == Some(program::MANUAL_PROGRAM_ID) {
            payload.push_str("Manually started ");
        } else {
            payload.push_str("Automatically scheduled ");
        }
        payload.push_str("Program ");
        payload.push_str(self.program_name.as_ref().unwrap_or(&String::from("")));
        payload.push_str(format!(" with {}% water level.", self.water_scale.unwrap_or(1.0)).as_str());

        payload
    }
}

impl WebHookEvent for super::BinarySensorEvent {
    fn ifttt_event(&self) -> String {
        format!("sensor{}", self.index)
    }

    fn payload(&self) -> String {
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

    fn payload(&self) -> String {
        format!("Flow count: {:.0}, volume: {:.2}", self.count, self.volume)
    }
}

impl WebHookEvent for super::RebootEvent {
    fn ifttt_event(&self) -> String {
        String::from("reboot")
    }

    fn payload(&self) -> String {
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

    fn payload(&self) -> String {
        let mut payload = String::new();
        payload.push_str(format!("Station {} ", self.station_name).as_str());
        payload.push_str(if self.state { "opened. " } else { "closed. " });

        if self.state == false && self.duration.is_some() {
            payload.push_str("It ran for ");
            payload.push_str(utils::duration_to_hms(self.duration.unwrap().num_seconds()).as_str());
        }

        if self.flow_volume.is_some() {
            payload.push_str(format!("Flow rate: {:.2}", self.flow_volume.unwrap()).as_str());
        }

        payload
    }
}

impl WebHookEvent for super::RainDelayEvent {
    fn ifttt_event(&self) -> String {
        String::from("rain_delay")
    }

    fn payload(&self) -> String {
        match self.state {
            false => String::from("Rain delay deactivated."),
            true => String::from("Rain delay activated."),
        }
    }
}

impl WebHookEvent for super::WaterScaleChangeEvent {
    fn ifttt_event(&self) -> String {
        "water_scale".into()
    }

    fn payload(&self) -> String {
        self.scale.to_string()
    }
}

impl WebHookEvent for super::IpAddrChangeEvent {
    fn ifttt_event(&self) -> String {
        "ip_address".into()
    }

    fn payload(&self) -> String {
        self.addr.to_string()
    }
}


