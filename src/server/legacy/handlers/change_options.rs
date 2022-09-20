use std::sync;

use crate::{
    opensprinkler::{config, sensor, station, weather, Controller},
    server::legacy::{self, error, values::options::MqttConfigJson, FromLegacyFormat},
};

use actix_web::{web, Responder, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ChangeOptionsRequest {
    /// **tz**: Timezone
    ///
    /// Range: 0–108
    #[serde(default, rename = "tz")]
    timezone: Option<u8>,
    /// **ntp**: Enable NTP
    #[serde(default, rename = "ntp", deserialize_with = "legacy::de::bool_from_int_option")]
    enable_ntp: Option<bool>,
    /// **dhcp**: Enable DHCP
    #[serde(default, rename = "dhcp", deserialize_with = "legacy::de::bool_from_int_option")]
    enable_dhcp: Option<bool>,
    /// **hp0**: HTTP Port (lower byte)
    ///
    /// Range: 0–255
    #[serde(default, rename = "hp0")]
    http_port_lower: Option<u8>,
    /// **hp1**: HTTP Port (upper byte)
    ///
    /// Range: 0–255
    #[serde(default, rename = "hp1")]
    http_port_upper: Option<u8>,
    /// **ext**: Extension board count
    ///
    /// Range: [0 – [station::MAX_EXT_BOARDS])
    #[serde(default, rename = "ext")]
    extension_boards: Option<usize>,
    /// **sdt**: Station delay time
    ///
    /// Range: [`-600` – `+600`]; step: `5`
    #[serde(default, rename = "sdt")]
    station_delay: Option<u8>,
    /// **mas**: Master station 1 - station index
    ///
    /// Range: [`0` – [station::MAX_NUM_STATIONS]]
    #[serde(default, rename = "mas")]
    master1_station: Option<usize>,
    /// **mton**: Master station 1 - adjusted on time
    ///
    /// Range: [`0` – `+600`]; step: `5`
    #[serde(default, rename = "mton")]
    master1_on: Option<i16>,
    /// **mtof**: Master station 1 - adjusted off time
    ///
    /// Range: [`-600` – `0`]; step: `5`
    #[serde(default, rename = "mtof")]
    master1_off: Option<i16>,
    /// **mas2**: Master station 2 - station index
    ///
    /// Range: [`0` – [station::MAX_NUM_STATIONS]]
    #[serde(default, rename = "mas2")]
    master2_station: Option<usize>,
    /// **mton2**: Master station 2 - adjusted on time
    ///
    /// Range: [`0` – `+600`]; step: `5`
    #[serde(default, rename = "mton2")]
    master2_on: Option<i16>,
    /// **mtof2**: Master station 2 - adjusted off time
    ///
    /// Range: [`-600` – `0`]; step: `5`
    #[serde(default, rename = "mtof2")]
    master2_off: Option<i16>,
    /// **wl**: Water level
    ///
    /// Range: [`0` – `250`]
    #[serde(default, rename = "wl")]
    watering: Option<u8>,
    /// **uwt**: Weather adjustment algorithm
    #[serde(default, rename = "uwt")]
    weather: Option<u8>,
    /// **lq**: Enable event log
    #[serde(default, rename = "lg", deserialize_with = "legacy::de::bool_from_int_option")]
    enable_log: Option<bool>,
    /// **fpr0**: Flow pulse rate (lower byte)
    ///
    /// Range: [`0` – `255`]
    #[serde(default, rename = "fpr0")]
    fpr0: Option<u8>,
    /// **fpr1**: Flow pulse rate (upper byte)
    ///
    /// Range: [`0` – `255`]
    #[serde(default, rename = "fpr1")]
    fpr1: Option<u8>,
    /// **sar**: Enable special station auto-refresh
    #[serde(default, rename = "sar", deserialize_with = "legacy::de::bool_from_int_option")]
    enable_special_stn_auto_refresh: Option<bool>,
    /// **ifkey**: IFTTT Web Hooks API key
    #[serde(default, rename = "ifkey")]
    ifttt_key: Option<String>,
    /// **ife**: Enable IFTTT events (bit field)
    #[serde(default, rename = "ife")]
    enable_ifttt_flags: Option<u8>,
    /// **sn1t**: Sensor 1 - type
    ///
    /// See: [sensor::SensorType]
    #[serde(default, rename = "sn1t")]
    sensor1_type: Option<u8>,
    /// **sn1o**: Sensor 1 - option
    ///
    /// * `0` = NC
    /// * `1` = NO
    #[serde(default, rename = "sn1o", deserialize_with = "legacy::de::bool_from_int_option")]
    sensor1_option: Option<bool>,
    /// **sn1on**: Sensor 1 - delay on
    ///
    /// Unit: minutes
    ///
    /// Range: [`0` – `255`]
    #[serde(default, rename = "sn1on")]
    sensor1_on: Option<u8>,
    /// **sn1of**: Sensor 1 - delay off
    ///
    /// Unit: minutes
    #[serde(default, rename = "sn1of")]
    sensor1_off: Option<u8>,
    /// **sn2t**: Sensor 2 - type
    ///
    /// See: [sensor::SensorType]
    #[serde(default, rename = "sn2t")]
    sensor2_type: Option<u8>,
    /// **sn2o**: Sensor 2 - option
    ///
    /// * `0` = NC
    /// * `1` = NO
    #[serde(default, rename = "sn2o", deserialize_with = "legacy::de::bool_from_int_option")]
    sensor2_option: Option<bool>,
    /// **sn2on**: Sensor 2 - delay on
    ///
    /// Unit: minutes
    ///
    /// Range: [`0` – `255`]
    #[serde(default, rename = "sn2on")]
    sensor2_on: Option<u8>,
    /// **sn2of**: Sensor 2 - delay off
    ///
    /// Unit: minutes
    ///
    /// Range: [`0` – `255`]
    #[serde(default, rename = "sn2of")]
    sensor2_off: Option<u8>,
    /// **loc**: Location
    ///
    /// Legacy firmware allowed city, region, postal code, PWS (personal weather station) ID, or GPS coordinates.
    /// **This firmware allows only GPS coordinates.**
    ///
    /// Format: `[latitude],[longitude]`
    #[serde(default, rename = "loc")]
    location: Option<String>,
    /// **wto**: Weather options
    ///
    /// ### JSON (URL-encoded, no braces)
    ///
    /// __Manual (0)__
    /// > n/a
    ///
    /// __Zimmerman (1)__
    /// > **t**: temperature, **h**: humidity, **r**: rain
    ///
    /// __Rain Delay (2)__
    /// > **d**: duration (hours)
    ///
    /// __ETo (3)__
    /// > **baseETo**: baseline ETo (in/day), **elevation**: elevation (ft)
    #[serde(default, rename = "wto")]
    weather_options: Option<String>,
    /// **mqtt**: MQTT Config
    ///
    /// JSON
    #[serde(default, rename = "mqtt")]
    mqtt_config: Option<String>,
    /// **ttt**: Set time
    ///
    /// Unit: seconds (epoch)
    #[serde(default, rename = "ttt")]
    set_time: Option<u64>,
}

/// URI: `/co`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<Controller>>>, parameters: web::Query<ChangeOptionsRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    let tracing_span = tracing::trace_span!("server_change_options");
    let _entered = tracing_span.enter();

    if let Some(timezone) = parameters.timezone {
        if timezone > 108 {
            tracing::trace!("INVALID tz: {}", timezone);

            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        open_sprinkler.config.timezone = timezone;
        tracing::trace!("tz: {}", timezone);
    }

    if let Some(enable_ntp) = parameters.enable_ntp {
        tracing::trace!("ntp: {}", enable_ntp);
    }

    if let Some(enable_dhcp) = parameters.enable_dhcp {
        tracing::trace!("dhcp: {}", enable_dhcp);
    }

    if let (Some(hp0), Some(hp1)) = (parameters.http_port_lower, parameters.http_port_upper) {
        let http_port: u16 = (hp1 as u16 * 256) + hp0 as u16;
        tracing::debug!("hp: {}", http_port);
    }

    if let Some(enable_log) = parameters.enable_log {
        open_sprinkler.config.enable_log = enable_log;
        tracing::trace!("lg: {}", enable_log);
    }

    if let Some(enable_special_stn_refresh) = parameters.enable_special_stn_auto_refresh {
        open_sprinkler.config.enable_special_stn_refresh = enable_special_stn_refresh;
        tracing::trace!("sar: {}", enable_special_stn_refresh);
    }

    if let Some(extension_board_count) = parameters.extension_boards {
        if extension_board_count > station::MAX_EXT_BOARDS {
            tracing::trace!("INVALID ext: {}", extension_board_count);
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        open_sprinkler.config.extension_board_count = extension_board_count;
        tracing::trace!("ext: {}", extension_board_count);
    }

    if let Some(station_delay_time) = parameters.station_delay {
        open_sprinkler.config.station_delay_time = station_delay_time;
        tracing::trace!("sdt: {}", station_delay_time);
    }

    if let Some(wl) = parameters.watering {
        let water_scale = f32::from(wl) / 100.0;

        if water_scale > weather::WATER_SCALE_MAX {
            tracing::trace!("INVALID wl: {}", wl);
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        open_sprinkler.config.water_scale = water_scale;
        tracing::trace!("wl: {}", wl);
    }

    if let Some(uwt) = parameters.weather {
        let algorithm = uwt & !(1 << 7);

        let _restriction = ((uwt >> 7) & 1) != 0; // @todo

        // @todo
        /* if open_sprinkler.config.weather.algorithm.unwrap().get_id() as u8 != algorithm {
            open_sprinkler.state.weather.request_update();
        } */

        open_sprinkler.config.weather.set_algorithm(Some(algorithm));
        tracing::trace!("uwt: {}", uwt);
    }

    if let Some(mas) = parameters.master1_station {
        if mas > station::MAX_NUM_STATIONS {
            tracing::trace!("INVALID mas: {}", mas);
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        if mas == 0 {
            open_sprinkler.config.master_stations[0].station = None;
        } else {
            open_sprinkler.config.master_stations[0].station = Some(mas - 1);
        }
        tracing::trace!("mas: {}", mas);
    }
    if let Some(mton) = parameters.master1_on {
        open_sprinkler.config.master_stations[0].set_adjusted_on_time_secs(mton);
        tracing::trace!("mton: {}", mton);
    }
    if let Some(mtof) = parameters.master1_off {
        open_sprinkler.config.master_stations[0].set_adjusted_off_time_secs(mtof);
        tracing::trace!("mtof: {}", mtof);
    }

    if let Some(mas2) = parameters.master2_station {
        if mas2 > station::MAX_NUM_STATIONS {
            tracing::trace!("INVALID mas2: {}", mas2);
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        if mas2 == 0 {
            open_sprinkler.config.master_stations[1].station = None;
        } else {
            open_sprinkler.config.master_stations[1].station = Some(mas2 - 1);
        }
        tracing::trace!("mas2: {}", mas2);
    }
    if let Some(mton2) = parameters.master2_on {
        open_sprinkler.config.master_stations[1].set_adjusted_on_time_secs(mton2);
        tracing::trace!("mton2: {}", mton2);
    }
    if let Some(mtof2) = parameters.master2_off {
        open_sprinkler.config.master_stations[1].set_adjusted_off_time_secs(mtof2);
        tracing::trace!("mtof2: {}", mtof2);
    }

    if let (Some(fpr0), Some(fpr1)) = (parameters.fpr0, parameters.fpr1) {
        let flow_pulse_rate = (((fpr1 as u16) << 8) + fpr0 as u16) / 100;
        open_sprinkler.config.flow_pulse_rate = flow_pulse_rate;
        tracing::trace!("fpr: {}", flow_pulse_rate);
    }

    if let Some(sensor_type) = parameters.sensor1_type {
        open_sprinkler.config.sensors[0].sensor_type = match sensor_type {
            //0 => Some(sensor::SensorType::None),
            0 => None,
            1 => Some(sensor::SensorType::Rain),
            2 => Some(sensor::SensorType::Flow),
            3 => Some(sensor::SensorType::Soil),
            240 => Some(sensor::SensorType::ProgramSwitch),
            255 => Some(sensor::SensorType::Other),
            _ => unimplemented!(),
        };
        tracing::trace!("sn1t: {}", sensor_type);
    }
    if let Some(normal_state) = parameters.sensor1_option {
        open_sprinkler.config.sensors[0].normal_state = match normal_state {
            false => sensor::NormalState::Closed,
            true => sensor::NormalState::Open,
        };
        tracing::trace!("sn1o: {}", if normal_state { 1 } else { 0 });
    }
    if let Some(delay_on) = parameters.sensor1_on {
        open_sprinkler.config.sensors[0].delay_on = delay_on;
        tracing::trace!("sn1on: {}", delay_on);
    }
    if let Some(delay_off) = parameters.sensor1_off {
        open_sprinkler.config.sensors[0].delay_off = delay_off;
        tracing::trace!("sn1of: {}", delay_off);
    }

    if let Some(sensor_type) = parameters.sensor2_type {
        open_sprinkler.config.sensors[1].sensor_type = match sensor_type {
            //0 => Some(sensor::SensorType::None),
            0 => None,
            1 => Some(sensor::SensorType::Rain),
            2 => Some(sensor::SensorType::Flow),
            3 => Some(sensor::SensorType::Soil),
            240 => Some(sensor::SensorType::ProgramSwitch),
            255 => Some(sensor::SensorType::Other),
            _ => unimplemented!(),
        };
        tracing::trace!("sn2t: {}", sensor_type);
    }
    if let Some(normal_state) = parameters.sensor2_option {
        open_sprinkler.config.sensors[1].normal_state = match normal_state {
            false => sensor::NormalState::Closed,
            true => sensor::NormalState::Open,
        };
        tracing::trace!("sn2o: {}", if normal_state { 1 } else { 0 });
    }
    if let Some(delay_on) = parameters.sensor2_on {
        open_sprinkler.config.sensors[1].delay_on = delay_on;
        tracing::trace!("sn2on: {}", delay_on);
    }
    if let Some(delay_off) = parameters.sensor2_off {
        open_sprinkler.config.sensors[1].delay_off = delay_off;
        tracing::trace!("sn2of: {}", delay_off);
    }

    if let Some(ref location_str) = parameters.location {
        if let Ok(location) = config::Location::try_from(location_str.as_ref()) {
            if open_sprinkler.config.location != location {
                open_sprinkler.state.weather.request_update();
            }

            open_sprinkler.config.location = location;
            tracing::trace!("loc: {}", location_str);
        } else {
            tracing::trace!("INVALID loc: {}", location_str);
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }
    }

    if let Some(ref wto) = parameters.weather_options {
        let options = Some(wto.to_string());
        if open_sprinkler.config.weather.options != options {
            open_sprinkler.state.weather.request_update();
        }

        open_sprinkler.config.weather.options = options;
        tracing::trace!("wto: {}", wto);
    }

    if let Some(ife) = parameters.enable_ifttt_flags {
        open_sprinkler.config.ifttt.events = config::EventsEnabled::from_legacy_format(ife);

        tracing::trace!("ife: {}", ife);
    }
    if let Some(ref ifkey) = parameters.ifttt_key {
        open_sprinkler.config.ifttt.web_hooks_key = ifkey.to_string();
        tracing::trace!("ifkey: {}", ifkey);
    }
    if let Some(ref mqtt_str) = parameters.mqtt_config {
        let mqtt_config: MqttConfigJson = serde_json::from_str(mqtt_str)?;

        open_sprinkler.config.mqtt.enabled = mqtt_config.enabled;
        open_sprinkler.config.mqtt.host = Some(mqtt_config.host);
        open_sprinkler.config.mqtt.port = mqtt_config.port;
        open_sprinkler.config.mqtt.username = Some(mqtt_config.user);
        open_sprinkler.config.mqtt.password = Some(mqtt_config.pass);

        tracing::trace!("mqtt: {}", mqtt_str);
    }
    if let Some(ttt) = parameters.set_time {
        let datetime = chrono::DateTime::<chrono::Utc>::from_utc(chrono::NaiveDateTime::from_timestamp(ttt as i64, 0), chrono::Utc);
        tracing::debug!("ttt: {}", datetime);
    }

    // if weather config has changed, reset state
    if open_sprinkler.state.weather.last_request_timestamp == None {
        open_sprinkler.config.set_water_scale(1.0);
        open_sprinkler.state.weather.raw_data = None;
        open_sprinkler.state.weather.last_response_code = Some(weather::ErrorCode::Unknown(-1));
    }

    Ok(error::ReturnErrorCode::Success)
}
