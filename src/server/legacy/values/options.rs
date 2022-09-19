use serde::{Serialize, Deserialize};

use crate::{server::legacy, opensprinkler::events};


#[derive(Serialize, Deserialize)]
pub struct MqttConfigJson {
    #[serde(rename = "en", serialize_with = "legacy::ser::int_from_bool", deserialize_with = "legacy::de::bool_from_int")]
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
}

impl From<events::mqtt::Config> for MqttConfigJson {
    fn from(config: events::mqtt::Config) -> Self {
        Self {
            enabled: config.enabled,
            host: config.host.unwrap_or_else(|| String::from("")),
            port: config.port,
            user: config.username.unwrap_or_else(|| String::from("")),
            pass: config.password.unwrap_or_else(|| String::from("")),
        }
    }
}