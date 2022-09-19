use reqwest::header;
use serde::{Serialize, Serializer};

use crate::opensprinkler::station;

include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

#[derive(Debug, Serialize)]
pub struct RemoteStationRequestParametersV2_1_9 {
    /// Device key (MD5)
    #[serde(rename = "pw")]
    device_key_md5: String,
    /// Station ID/index
    #[serde(rename = "sid")]
    station: station::StationIndex,
    /// Enable bit
    #[serde(rename = "en", serialize_with = "char_from_bool")]
    value: bool,
    /// Timer (seconds)
    #[serde(rename = "t")]
    timer: i64,
}

impl RemoteStationRequestParametersV2_1_9 {
    pub fn new(device_key_md5: &str, station: station::StationIndex, value: bool, timer: i64) -> Self {
        RemoteStationRequestParametersV2_1_9 {
            device_key_md5: device_key_md5.to_string(),
            station,
            value,
            timer,
        }
    }
}

fn char_from_bool<S>(x: &bool, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_char(match x {
        true => '1',
        false => '0',
    })
}

const USER_AGENT_VALUE: header::HeaderValue = header::HeaderValue::from_static(constants::USER_AGENT_STRING);

pub fn build_client() -> reqwest::Result<reqwest::blocking::Client> {
    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, USER_AGENT_VALUE);

    Ok(reqwest::blocking::Client::builder().default_headers(headers).build()?)
}