use core::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    EncodeError(rmp_serde::encode::Error),
    DecodeError(rmp_serde::decode::Error),
    SerdeError(serde_json::Error),
    
    MqttError(paho_mqtt::Error),
    IftttRequestError(reqwest::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::IoError(ref err) => write!(f, "IO Error: {:?}", err),
            Self::EncodeError(ref err) => write!(f, "Encode Error: {:?}", err),
            Self::DecodeError(ref err) => write!(f, "Decode Error: {:?}", err),
            Self::SerdeError(ref err) => write!(f, "Serde Error: {:?}", err),
            
            Self::MqttError(ref err) => write!(f, "MQTT Error: {:?}", err),
            Self::IftttRequestError(ref err) => write!(f, "IFTTT Webhook Error: {:?}", err),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(err: rmp_serde::encode::Error) -> Self {
        Self::EncodeError(err)
    }
}

impl From<rmp_serde::decode::Error> for Error {
    fn from(err: rmp_serde::decode::Error) -> Self {
        Self::DecodeError(err)
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
