use super::{log, OpenSprinkler};
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
};

#[derive(Default)]
pub struct WeatherStatus {
    /// time when weather was checked (seconds)
    pub checkwt_lasttime: Option<i64>,

    /// time when weather check was successful (seconds)
    pub checkwt_success_lasttime: Option<i64>,

    /// Result of the most recent request to the weather service
    pub last_response_code: Option<i8>,

    /// Data returned by the weather service (used by web server)
    pub raw_data: Option<String>,
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
        UrlParse(url::ParseError)
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
        fn from(err: url::ParseError) -> Error {
            Error::UrlParse(err)
        }
    }
}

/// Make weather query
pub fn check_weather(open_sprinkler: &mut OpenSprinkler, update_fn: &dyn Fn(&OpenSprinkler, WeatherUpdateFlag)) -> result::Result<()> {
    // Skip checking weather if a) network has failed; or b) controller is configured as remote extender
    if !open_sprinkler.network_connected() || open_sprinkler.is_remote_extension() {
        return Ok(());
    }

    // Skip checking weather if program is active
    if open_sprinkler.status.program_busy {
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

        //if !(open_sprinkler.iopts.uwt == 0 || open_sprinkler.iopts.uwt == 2) {
        if !(open_sprinkler.controller_config.uwt == 0 || open_sprinkler.controller_config.uwt == 2) {
            open_sprinkler.set_water_scale(100); // reset watering percentage to 100%
            open_sprinkler.weather_status.raw_data = None; // reset wt_rawData and errCode
            open_sprinkler.weather_status.last_response_code = None;
        }
    } else if open_sprinkler.weather_status.checkwt_lasttime.is_none() || now > open_sprinkler.weather_status.checkwt_lasttime.unwrap() + CHECK_WEATHER_TIMEOUT {
        open_sprinkler.weather_status.checkwt_lasttime = Some(now);
        return Ok(get_weather(open_sprinkler, update_fn)?);
    }

    Ok(())
}

fn get_weather(open_sprinkler: &mut OpenSprinkler, update_fn: &dyn Fn(&OpenSprinkler, WeatherUpdateFlag)) -> result::Result<()> {
    // @todo use semver and cargo cfg version
    //let ua = HeaderValue::try_from(format!("OpenSprinkler/{} (rust)", open_sprinkler.iopts.fwv));
    let ua = HeaderValue::try_from(format!("OpenSprinkler/{} (rust)", open_sprinkler.controller_config.fwv));

    //let mut url = reqwest::Url::parse(open_sprinkler.sopts.wsp.as_str()).unwrap();
    //url.path_segments_mut().unwrap().push(&open_sprinkler.iopts.uwt.to_string());
    let mut url = open_sprinkler.get_weather_service_url()?;

    tracing::debug!("Retrieving weather from {}", url.host_str().unwrap_or(""));

    let client = reqwest::blocking::Client::new();
    // @todo log request failures, handle request failures
    let response = client
        .get(url)
        .header(USER_AGENT, ua.unwrap())
        // Prefer JSON over the original implementation that returned a form-urlencoded string
        .header(ACCEPT, HeaderValue::from_str("application/json,text/plain;q=0.9,text/html;q=0.8").unwrap())
        .query(&[("loc", open_sprinkler.controller_config.sopts.loc.clone()), ("wto", open_sprinkler.controller_config.sopts.wto.clone()), ("fwv", open_sprinkler.controller_config.fwv.to_string())])
        .send()
        .expect("Error making HTTP weather request");

    tracing::debug!("Received HTTP {} from {}", response.status(), response.url());

    let content_type = String::from(response.headers().get(CONTENT_TYPE).unwrap_or(&HeaderValue::from_str("text/plain").unwrap()).to_str().unwrap_or(""));

    if content_type.starts_with("application/json") {
        // Parsing JSON is easier!
        todo!()
    } else if content_type.starts_with("text/plain") || content_type.starts_with("text/html") {
        // Original weather service response format
        let params: HashMap<String, String> = form_urlencoded::parse(response.text().unwrap_or("".to_string()).as_str().as_bytes()).into_owned().collect();

        let mut save_nvdata = false;

        // first check errCode, only update lswc timestamp if errCode is 0
        open_sprinkler.weather_status.last_response_code = None;
        if params.contains_key("errCode") {
            let err_code = params.get("errCode").unwrap_or(&String::from("")).parse::<i8>();

            if err_code.is_ok() {
                open_sprinkler.weather_status.last_response_code = Some(err_code.unwrap());

                tracing::debug!("Weather service returned response code: {}", open_sprinkler.weather_status.last_response_code.unwrap());

                if open_sprinkler.weather_status.last_response_code.unwrap() == 0 {
                    open_sprinkler.set_check_weather_success_timestamp();
                }
            }
        }

        // Watering Level (scale)
        if open_sprinkler.weather_status.last_response_code.is_some() && open_sprinkler.weather_status.last_response_code.unwrap() == 0 && params.contains_key("scale") {
            let scale = params.get("scale").unwrap().parse::<i32>().unwrap_or(-1);
            //if scale >= 0 && scale <= WATER_SCALE_MAX && scale != open_sprinkler.iopts.wl as i32 {
            if scale >= 0 && scale <= WATER_SCALE_MAX && scale != open_sprinkler.get_water_scale() as i32 {
                // Only save if the value has changed
                //open_sprinkler.iopts.wl = u8::try_from(scale).unwrap();
                open_sprinkler.controller_config.wl = u8::try_from(scale).unwrap();
                open_sprinkler.iopts_save();
                update_fn(open_sprinkler, WeatherUpdateFlag::WL);

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
                open_sprinkler.controller_config.sunrise_time = u16::try_from(sunrise).unwrap();
                save_nvdata = true;
                update_fn(open_sprinkler, WeatherUpdateFlag::SUNRISE);

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
                open_sprinkler.controller_config.sunset_time = u16::try_from(sunset).unwrap();
                save_nvdata = true;
                update_fn(open_sprinkler, WeatherUpdateFlag::SUNSET);

                tracing::trace!("Sunset: {}", open_sprinkler.get_sunset_time());
            }
        }

        // External IP
        // @todo IPv6 support
        if params.contains_key("eip") {
            let ip_uint = params.get("eip").unwrap().parse::<u32>();
            if ip_uint.is_ok() {
                let eip = Ipv4Addr::from(ip_uint.unwrap());
                //if open_sprinkler.nvdata.external_ip.is_none() || (open_sprinkler.nvdata.external_ip.is_some() && eip != open_sprinkler.nvdata.external_ip.unwrap()) {
                if eip != open_sprinkler.controller_config.external_ip.unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))) {
                    // Only save if the value has changed
                    //open_sprinkler.nvdata.external_ip = Some(std::net::IpAddr::V4(eip));
                    open_sprinkler.controller_config.external_ip = Some(std::net::IpAddr::V4(eip));
                    save_nvdata = true;
                    update_fn(open_sprinkler, WeatherUpdateFlag::EIP);

                    tracing::trace!("External IP: {}", eip);
                }
            }
        }

        // Timezone
        if params.contains_key("tz") {
            let tz = params.get("tz").unwrap().parse::<i8>().unwrap();
            //if tz >= 0 && tz <= 108 && tz != open_sprinkler.iopts.tz as i8 {
            if tz >= 0 && tz <= 108 && tz != open_sprinkler.controller_config.tz as i8 {
                //open_sprinkler.iopts.tz = u8::try_from(tz).unwrap();
                open_sprinkler.controller_config.tz = u8::try_from(tz).unwrap();
                open_sprinkler.iopts_save();
                update_fn(open_sprinkler, WeatherUpdateFlag::TZ);

                tracing::trace!("Timezone: {}", open_sprinkler.controller_config.tz);
            }
        }

        // Rain delay (returned as hours)
        if params.contains_key("rd") {
            let rd = params.get("rd").unwrap().parse::<i64>();
            if rd.is_ok() {
                let rd = rd.unwrap();

                if rd > 0 {
                    //open_sprinkler.nvdata.rd_stop_time = Some((chrono::Utc::now() + chrono::Duration::hours(rd)).timestamp());
                    open_sprinkler.controller_config.rd_stop_time = Some((chrono::Utc::now() + chrono::Duration::hours(rd)).timestamp());
                    open_sprinkler.rain_delay_start();
                    update_fn(open_sprinkler, WeatherUpdateFlag::RD);
                    tracing::trace!("Starting rain delay for: {}h", rd);
                } else if rd == 0 {
                    open_sprinkler.rain_delay_stop();
                    update_fn(open_sprinkler, WeatherUpdateFlag::RD);
                    tracing::trace!("Ending rain delay");
                }
            }
        }

        // Raw Data
        if params.contains_key("rawData") {
            let raw_data = params.get("rawData").unwrap();
            tracing::trace!("Raw data: {}", raw_data);
            // @todo Store raw_data in memory for web server
        }

        // Save non-volatile data, if necessary
        if save_nvdata {
            open_sprinkler.nvdata_save();
        }

        let _ = log::write_log_message(
            &open_sprinkler,
            &log::message::WaterLevelMessage::new(open_sprinkler.get_water_scale(), open_sprinkler.weather_status.checkwt_success_lasttime.unwrap()),
            open_sprinkler.weather_status.checkwt_success_lasttime.unwrap(),
        );
    }

    Ok(())
}
