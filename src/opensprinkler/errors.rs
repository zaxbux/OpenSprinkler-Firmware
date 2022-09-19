use core::fmt;
use std::error::Error;

use super::config;

pub type Result<T> = core::result::Result<T, SetupError>;

#[derive(Debug)]
pub enum SetupError {
	ConfigError(config::result::Error),
	
	MqttError(paho_mqtt::Error),
}

impl From<config::result::Error> for SetupError {
	fn from(err: config::result::Error) -> Self {
		SetupError::ConfigError(err)
	}
}


impl From<paho_mqtt::Error> for SetupError {
	fn from(err: paho_mqtt::Error) -> Self {
		SetupError::MqttError(err)
	}
}

impl fmt::Display for SetupError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
    		SetupError::ConfigError(err) => write!(f, "Configuration error: {:?}", err),
			
    		SetupError::MqttError(err) => write!(f, "MQTT error: {:?}", err),
		}
	}
}

impl Error for SetupError {}