use serde::{Serialize, Serializer};

use crate::opensprinkler::StationIndex;

#[derive(Debug, Serialize)]
pub struct RemoteStationRequestParametersV219 {
    /// Device key (MD5)
    #[serde(rename = "pw")]
    device_key_md5: String,
    /// Station ID/index
    #[serde(rename = "sid")]
    station: StationIndex,
    /// Enable bit
    #[serde(rename = "en", serialize_with = "char_from_bool")]
    value: bool,
    /// Timer (seconds)
    #[serde(rename = "t")]
    timer: i64,
}

impl RemoteStationRequestParametersV219 {
    pub fn new(device_key_md5: &str, station: StationIndex, value: bool, timer: i64) -> Self {
        RemoteStationRequestParametersV219 {
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