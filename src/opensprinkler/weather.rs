pub mod algorithm;

use crate::opensprinkler::{http::request, events};

use super::{log, OpenSprinkler};
use core::fmt;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use serde::{de, ser, Deserialize, Deserializer, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
};

pub type WeatherServiceRawData = Option<serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConfig {
    /// Weather Service URL
    pub service_url: String,
    /// Weather algorithm
    pub algorithm: Option<WeatherAlgorithm>,
    /// Weather adjustment options
    ///
    /// This data is specific to the weather adjustment method.
    pub options: Option<String>,
}

impl WeatherConfig {
    pub fn set_algorithm(&mut self, id: Option<u8>) {
        match id {
            None => self.algorithm = None,
            Some(0) => self.algorithm = Some(WeatherAlgorithm::Manual(algorithm::Manual)),
            Some(1) => self.algorithm = Some(WeatherAlgorithm::Zimmerman(algorithm::Zimmerman)),
            Some(2) => self.algorithm = Some(WeatherAlgorithm::RainDelay(algorithm::RainDelay)),
            Some(3) => self.algorithm = Some(WeatherAlgorithm::ETo(algorithm::Evapotranspiration)),
            _ => unimplemented!(),
        }
    }
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            service_url: core::option_env!("WEATHER_SERVICE_URL").unwrap_or("https://weather.opensprinkler.com").into(),
            algorithm: None,
            options: None,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WeatherAlgorithmID {
    Manual = 0,
    Zimmerman = 1,
    RainDelay = 2,
    ETo = 3,
}

impl ToString for WeatherAlgorithmID {
    fn to_string(&self) -> String {
        match *self {
            WeatherAlgorithmID::Manual => 0.to_string(),
            WeatherAlgorithmID::Zimmerman => 1.to_string(),
            WeatherAlgorithmID::RainDelay => 2.to_string(),
            WeatherAlgorithmID::ETo => 3.to_string(),
        }
    }
}

impl Into<i8> for WeatherAlgorithmID {
    fn into(self) -> i8 {
        match self {
            WeatherAlgorithmID::Manual => 0,
            WeatherAlgorithmID::Zimmerman => 1,
            WeatherAlgorithmID::RainDelay => 2,
            WeatherAlgorithmID::ETo => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WeatherAlgorithm {
    Manual(algorithm::Manual),
    Zimmerman(algorithm::Zimmerman),
    RainDelay(algorithm::RainDelay),
    ETo(algorithm::Evapotranspiration),
}

impl WeatherAlgorithm {
    pub fn get_id(&self) -> WeatherAlgorithmID {
        match self {
            WeatherAlgorithm::Manual(_) => WeatherAlgorithmID::Manual,
            WeatherAlgorithm::Zimmerman(_) => WeatherAlgorithmID::Zimmerman,
            WeatherAlgorithm::RainDelay(_) => WeatherAlgorithmID::RainDelay,
            WeatherAlgorithm::ETo(_) => WeatherAlgorithmID::ETo,
        }
    }

    pub fn use_manual_scale(&self) -> bool {
        match self {
            WeatherAlgorithm::Manual(a) => algorithm::WeatherAlgorithm::use_manual_scale(a),
            WeatherAlgorithm::Zimmerman(a) => algorithm::WeatherAlgorithm::use_manual_scale(a),
            WeatherAlgorithm::RainDelay(a) => algorithm::WeatherAlgorithm::use_manual_scale(a),
            WeatherAlgorithm::ETo(a) => algorithm::WeatherAlgorithm::use_manual_scale(a),
        }
    }
}

impl ser::Serialize for WeatherAlgorithm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            WeatherAlgorithm::Manual(_) => serializer.serialize_i8(WeatherAlgorithmID::Manual.into()),
            WeatherAlgorithm::Zimmerman(_) => serializer.serialize_i8(WeatherAlgorithmID::Zimmerman.into()),
            WeatherAlgorithm::RainDelay(_) => serializer.serialize_i8(WeatherAlgorithmID::RainDelay.into()),
            WeatherAlgorithm::ETo(_) => serializer.serialize_i8(WeatherAlgorithmID::ETo.into()),
        }
    }
}

struct WeatherAlgorithmVisitor;

impl<'de> de::Visitor<'de> for WeatherAlgorithmVisitor {
    type Value = i32;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an integer between -2^7 and 2^7")
    }

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(v)
    }
}

impl<'de> de::Deserialize<'de> for WeatherAlgorithm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match deserializer.deserialize_i32(WeatherAlgorithmVisitor)? {
            0 => Ok(WeatherAlgorithm::Manual(algorithm::Manual)),
            1 => Ok(WeatherAlgorithm::Zimmerman(algorithm::Zimmerman)),
            2 => Ok(WeatherAlgorithm::RainDelay(algorithm::RainDelay)),
            3 => Ok(WeatherAlgorithm::ETo(algorithm::Evapotranspiration)),
            _ => unimplemented!(),
        }
    }
}

#[derive(Default)]
pub struct WeatherStatus {
    /// time when weather was checked (seconds)
    pub checkwt_lasttime: Option<i64>,

    /// time when weather check was successful (seconds)
    pub checkwt_success_lasttime: Option<i64>,

    /// Result of the most recent request to the weather service
    pub last_response_code: Option<ErrorCode>,

    /// Data returned by the weather service (used by web server)
    pub raw_data: WeatherServiceRawData,
}

impl WeatherStatus {
    pub fn last_response_was_successful(&self) -> bool {
        self.last_response_code == Some(ErrorCode::Success)
    }
}

#[repr(u8)]
pub enum WeatherUpdateFlag {
    SUNRISE = 0x01,
    SUNSET = 0x02,
    EIP = 0x04,
    WL = 0x08,
    TZ = 0x10,
    RD = 0x20,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum ErrorCode {
    Success,
    // @todo unify error codes between Firmware, Weather, and GUI.
    Unknown(i8),
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Success => write!(f, "Success"),
            ErrorCode::Unknown(ref code) => write!(f, "Unknown weather service error: {}", code),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WeatherServiceResponse {
    #[serde(rename(deserialize = "errCode"), deserialize_with = "weather_service_error_code")]
    error_code: ErrorCode,

    scale: Option<u8>,

    sunrise: Option<u16>,

    sunset: Option<u16>,

    #[serde(rename(deserialize = "externalIP"))]
    external_ip: Option<IpAddr>,

    timezone: Option<u8>,

    #[serde(rename(deserialize = "rainDelay"))]
    rain_delay: Option<u8>,

    #[serde(rename(deserialize = "rawData"))]
    raw_data: WeatherServiceRawData,
}

/// Temporary decoder until error codes are unified.
fn weather_service_error_code<'de, D>(deserializer: D) -> Result<ErrorCode, D::Error>
where
    D: Deserializer<'de>,
{
    let code = i8::deserialize(deserializer)?;
    Ok(match code {
        0 => ErrorCode::Success,
        _ => ErrorCode::Unknown(code),
    })
}

/// Weather check interval (seconds)
pub const CHECK_WEATHER_TIMEOUT: i64 = 21613; // approx 360 minutes
/// Weather check success interval (seconds)
pub const CHECK_WEATHER_SUCCESS_TIMEOUT: i64 = 86400; // 24 hours

pub const WATER_SCALE_MAX: i32 = 250;

mod result {
    use std::{error, fmt, result};

    pub type Result<T> = result::Result<T, Error>;

    #[derive(Debug)]
    pub enum Error {
        UrlParse(url::ParseError),
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                Error::UrlParse(ref err) => write!(f, "URL Parser error: {}", err),
            }
        }
    }

    impl error::Error for Error {}

    impl From<url::ParseError> for Error {
        fn from(err: url::ParseError) -> Self {
            Error::UrlParse(err)
        }
    }
}

/// Make weather query
pub fn check_weather(open_sprinkler: &mut OpenSprinkler) -> result::Result<()> {
    // Skip checking weather if a) network has failed; or b) controller is configured as remote extender
    if !open_sprinkler.network_connected() || open_sprinkler.is_remote_extension() {
        return Ok(());
    }

    // Skip checking weather if program is active
    if open_sprinkler.status_current.program_busy {
        return Ok(());
    }

    let now = chrono::Utc::now().timestamp();

    if open_sprinkler.weather_status.checkwt_success_lasttime.is_some() && now > open_sprinkler.weather_status.checkwt_success_lasttime.unwrap() + CHECK_WEATHER_SUCCESS_TIMEOUT {
        // if last successful weather call timestamp is more than allowed threshold
        // and if the selected adjustment method is not manual
        // reset watering percentage to 100
        // TODO: the firmware currently needs to be explicitly aware of which adjustment methods
        // use manual watering percentage (namely methods 0 and 2), this is not ideal
        open_sprinkler.weather_status.checkwt_success_lasttime = None;

        //if open_sprinkler.config.weather.algorithm.is_some() && open_sprinkler.config.weather.algorithm != Some(WeatherAlgorithmID::RainDelay) {
        if let Some(algorithm) = &open_sprinkler.config.weather.algorithm {
            if !algorithm.use_manual_scale() {
                open_sprinkler.set_water_scale(100); // reset watering percentage to 100%
                open_sprinkler.weather_status.raw_data = None; // reset wt_rawData and errCode
                open_sprinkler.weather_status.last_response_code = None;
            }
        }
    } else if open_sprinkler.weather_status.checkwt_lasttime.is_none() || now > open_sprinkler.weather_status.checkwt_lasttime.unwrap() + CHECK_WEATHER_TIMEOUT {
        open_sprinkler.weather_status.checkwt_lasttime = Some(now);
        return Ok(get_weather(open_sprinkler)?);
    }

    Ok(())
}

fn get_weather(open_sprinkler: &mut OpenSprinkler) -> result::Result<()> {
    let url = open_sprinkler.get_weather_service_url()?;

    if url.is_none() {
        // Weather requests are disabled.
        return Ok(());
    }

    let url = url.unwrap();

    tracing::debug!("Retrieving weather from {:?}", url.host_str());

    let client = request::build_client().unwrap();
    // @todo log request failures, handle request failures
    let response = client
        .get(url)
        // Prefer JSON over the original implementation that returned a form-urlencoded string
        .header(ACCEPT, HeaderValue::from_str("application/json,text/plain;q=0.9,text/html;q=0.8").unwrap())
        .query(&[
            ("loc", &open_sprinkler.config.location.to_string()),
            ("wto", open_sprinkler.config.weather.options.as_ref().unwrap_or(&String::from(""))),
            ("fwv", &open_sprinkler.config.firmware_version.to_string()), // @todo Is this still necessary if it is included in the User-Agent header?
        ])
        .send()
        .expect("Error making HTTP weather request");

    tracing::debug!("Received HTTP {} from {}", response.status(), response.url());

    let content_type = String::from(response.headers().get(CONTENT_TYPE).unwrap_or(&HeaderValue::from_str("text/plain").unwrap()).to_str().unwrap_or(""));

    if content_type.starts_with("application/json") {
        let json = response.json::<WeatherServiceResponse>();

        if let Err(error) = json {
            tracing::warn!("Could not parse JSON response: {:?}", error);
            return Ok(());
        }

        let data = json.unwrap();

        open_sprinkler.weather_status.last_response_code = Some(data.error_code);

        tracing::debug!("Weather service returned response code: {}", open_sprinkler.weather_status.last_response_code.as_ref().unwrap());

        if open_sprinkler.weather_status.last_response_was_successful() {
            open_sprinkler.update_check_weather_success_timestamp();
        }

        if open_sprinkler.weather_status.last_response_was_successful() {
            if let Some(scale) = data.scale {
                if scale <= WATER_SCALE_MAX as u8 && scale != open_sprinkler.get_water_scale() {
                    open_sprinkler.config.water_scale = scale;

                    events::push_message(open_sprinkler, &events::WeatherUpdateEvent::water_scale(scale));

                    tracing::trace!("Watering scale: {}", open_sprinkler.get_water_scale());
                }
            }
        }

        if let Some(sunrise) = data.sunrise {
            if sunrise <= 1440 && sunrise != open_sprinkler.get_sunrise_time() {
                open_sprinkler.config.sunrise_time = sunrise;
                tracing::trace!("Sunrise: {}", open_sprinkler.get_sunrise_time());
            }
        }

        if let Some(sunset) = data.sunset {
            if sunset <= 1440 && sunset != open_sprinkler.get_sunset_time() {
                open_sprinkler.config.sunset_time = sunset;
                tracing::trace!("Sunset: {}", open_sprinkler.get_sunset_time());
            }
        }

        if let Some(external_ip) = data.external_ip {
            if Some(external_ip) != open_sprinkler.config.external_ip {
                open_sprinkler.config.external_ip = Some(external_ip);

                events::push_message(open_sprinkler, &events::WeatherUpdateEvent::external_ip(external_ip));

                tracing::trace!("External IP: {}", open_sprinkler.config.external_ip.unwrap());
            }
        }

        if let Some(timezone) = data.timezone {
            if timezone <= 108 && timezone != open_sprinkler.config.timezone {
                open_sprinkler.config.timezone = timezone;
                tracing::trace!("Timezone: {}", open_sprinkler.config.timezone);
            }
        }

        if let Some(rain_delay) = data.rain_delay {
            if rain_delay > 0 {
                open_sprinkler.config.rain_delay_stop_time = Some((chrono::Utc::now() + chrono::Duration::hours(i64::from(rain_delay))).timestamp());
                open_sprinkler.rain_delay_start();
                tracing::trace!("Starting rain delay for: {}h", rain_delay);
            } else if rain_delay == 0 {
                open_sprinkler.rain_delay_stop();
                tracing::trace!("Ending rain delay");
            }
        }

        if let Some(raw_data) = data.raw_data {
            open_sprinkler.weather_status.raw_data = Some(raw_data);
        }

        open_sprinkler.config.write().unwrap();

        let _ = log::write_log_message(
            &open_sprinkler,
            &log::message::WaterLevelMessage::new(open_sprinkler.get_water_scale(), open_sprinkler.weather_status.checkwt_success_lasttime.unwrap()),
            open_sprinkler.weather_status.checkwt_success_lasttime.unwrap(),
        );

        return Ok(());
    } else if content_type.starts_with("text/plain") || content_type.starts_with("text/html") {
        // Original weather service response format
        let params: HashMap<String, String> = form_urlencoded::parse(response.text().unwrap_or("".to_string()).as_str().as_bytes()).into_owned().collect();

        let mut save_nvdata = false;

        // first check errCode, only update lswc timestamp if errCode is 0
        open_sprinkler.weather_status.last_response_code = None;
        if params.contains_key("errCode") {
            let err_code = params.get("errCode").unwrap_or(&String::from("")).parse::<i8>();

            if err_code.is_ok() {
                let err_code = err_code.unwrap();
                open_sprinkler.weather_status.last_response_code = Some(match err_code {
                    0 => ErrorCode::Success,
                    _ => ErrorCode::Unknown(err_code),
                });

                tracing::debug!("Weather service returned response code: {}", err_code);

                if open_sprinkler.weather_status.last_response_was_successful() {
                    open_sprinkler.update_check_weather_success_timestamp();
                }
            }
        }

        // Watering Level (scale)
        if open_sprinkler.weather_status.last_response_was_successful() && params.contains_key("scale") {
            let scale = params.get("scale").unwrap().parse::<i32>().unwrap_or(-1);
            //if scale >= 0 && scale <= WATER_SCALE_MAX && scale != open_sprinkler.iopts.wl as i32 {
            if scale >= 0 && scale <= WATER_SCALE_MAX && scale != open_sprinkler.get_water_scale() as i32 {
                // Only save if the value has changed
                //open_sprinkler.iopts.wl = u8::try_from(scale).unwrap();
                open_sprinkler.config.water_scale = u8::try_from(scale).unwrap();
                open_sprinkler.config.write().unwrap();

                events::push_message(&open_sprinkler, &events::WeatherUpdateEvent::new(None, open_sprinkler.config.external_ip));

                tracing::trace!("Watering scale: {}", open_sprinkler.get_water_scale());
            }
        }

        // Sunrise time
        if params.contains_key("sunrise") {
            let sunrise = params.get("sunrise").unwrap().parse::<i16>().unwrap();
            //if sunrise >= 0 && sunrise <= 1440 && sunrise != open_sprinkler.nvdata.sunrise_time as i16 {
            if sunrise >= 0 && sunrise <= 1440 && sunrise != open_sprinkler.get_sunrise_time() as i16 {
                // Only save if the value has changed
                //open_sprinkler.nvdata.sunrise_time = u16::try_from(sunrise).unwrap();
                open_sprinkler.config.sunrise_time = u16::try_from(sunrise).unwrap();
                save_nvdata = true;

                tracing::trace!("Sunrise: {}", open_sprinkler.get_sunrise_time());
            }
        }

        // Sunset time
        if params.contains_key("sunset") {
            let sunset = params.get("sunset").unwrap().parse::<i16>().unwrap();
            //if sunset >= 0 && sunset <= 1440 && sunset != open_sprinkler.nvdata.sunset_time as i16 {
            if sunset >= 0 && sunset <= 1440 && sunset != open_sprinkler.get_sunset_time() as i16 {
                // Only save if the value has changed
                //open_sprinkler.nvdata.sunset_time = u16::try_from(sunset).unwrap();
                open_sprinkler.config.sunset_time = u16::try_from(sunset).unwrap();
                save_nvdata = true;

                tracing::trace!("Sunset: {}", open_sprinkler.get_sunset_time());
            }
        }

        // External IP
        if params.contains_key("eip") {
            let ip_uint = params.get("eip").unwrap().parse::<u32>();
            if ip_uint.is_ok() {
                let eip = Ipv4Addr::from(ip_uint.unwrap());
                //if open_sprinkler.nvdata.external_ip.is_none() || (open_sprinkler.nvdata.external_ip.is_some() && eip != open_sprinkler.nvdata.external_ip.unwrap()) {
                if eip != open_sprinkler.config.external_ip.unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))) {
                    // Only save if the value has changed
                    //open_sprinkler.nvdata.external_ip = Some(std::net::IpAddr::V4(eip));
                    open_sprinkler.config.external_ip = Some(std::net::IpAddr::V4(eip));
                    save_nvdata = true;
                    events::push_message(&open_sprinkler, &events::WeatherUpdateEvent::new(Some(open_sprinkler.config.water_scale), None));

                    tracing::trace!("External IP: {}", eip);
                }
            }
        }

        // Timezone
        if params.contains_key("tz") {
            let tz = params.get("tz").unwrap().parse::<i8>().unwrap();
            //if tz >= 0 && tz <= 108 && tz != open_sprinkler.iopts.tz as i8 {
            if tz >= 0 && tz <= 108 && tz != open_sprinkler.config.timezone as i8 {
                //open_sprinkler.iopts.tz = u8::try_from(tz).unwrap();
                open_sprinkler.config.timezone = u8::try_from(tz).unwrap();
                open_sprinkler.config.write().unwrap();

                tracing::trace!("Timezone: {}", open_sprinkler.config.timezone);
            }
        }

        // Rain delay (returned as hours)
        if params.contains_key("rd") {
            let rd = params.get("rd").unwrap().parse::<i64>();
            if rd.is_ok() {
                let rd = rd.unwrap();

                if rd > 0 {
                    //open_sprinkler.nvdata.rd_stop_time = Some((chrono::Utc::now() + chrono::Duration::hours(rd)).timestamp());
                    open_sprinkler.config.rain_delay_stop_time = Some((chrono::Utc::now() + chrono::Duration::hours(rd)).timestamp());
                    open_sprinkler.rain_delay_start();
                    tracing::trace!("Starting rain delay for: {}h", rd);
                } else if rd == 0 {
                    open_sprinkler.rain_delay_stop();
                    tracing::trace!("Ending rain delay");
                }
            }
        }

        // Raw Data
        if params.contains_key("rawData") {
            if let Some(raw_data) = params.get("rawData") {
                if let Ok(raw_data) = serde_json::from_str(raw_data) {
                    open_sprinkler.weather_status.raw_data = Some(raw_data);
                }
            }
        }

        // Save non-volatile data, if necessary
        if save_nvdata {
            open_sprinkler.config.write().unwrap();
        }

        let _ = log::write_log_message(
            &open_sprinkler,
            &log::message::WaterLevelMessage::new(open_sprinkler.get_water_scale(), open_sprinkler.weather_status.checkwt_success_lasttime.unwrap()),
            open_sprinkler.weather_status.checkwt_success_lasttime.unwrap(),
        );
    }

    Ok(())
}
