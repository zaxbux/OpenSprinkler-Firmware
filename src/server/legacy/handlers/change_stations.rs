use crate::{
    opensprinkler::{station, OpenSprinkler},
    server::legacy::error,
};

use actix_web::{web, HttpRequest, Responder, Result};
use serde::Deserialize;
use std::{collections::HashMap, sync};

#[derive(Debug, Deserialize)]
pub struct ChangeSpecialStationRequest {
    #[serde(rename = "sid")]
    station_index: usize,
    #[serde(rename = "st")]
    station_type: u8,
    #[serde(rename = "sd")]
    data: String,
}

/// Change station data.
///
/// URI: `/cs`
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<OpenSprinkler>>>, parameters: web::Query<HashMap<String, String>>, req: HttpRequest) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    // names
    for station_index in 0..station::MAX_NUM_STATIONS {
        if let Some(name) = parameters.get(&format!("s{}", station_index)) {
            open_sprinkler.config.stations[station_index].name = name.to_owned();
        }
    }

    // disable
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'd', |station_index, value| {
        tracing::trace!("set is_disabled @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.is_disabled = value;
    });

    // special
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'p', |station_index, value| {
        tracing::trace!("set is_special @ station_{} = {}", station_index, value);
        if !value {
            open_sprinkler.config.stations[station_index].station_type = station::StationType::Standard;
            open_sprinkler.config.stations[station_index].sped = None;
        }
    });

    if let Ok(special) = web::Query::<ChangeSpecialStationRequest>::from_query(req.query_string()) {
        if special.station_index > open_sprinkler.get_station_count() {
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }

        let mut special_data = station::SpecialStationData::NONE(());

        //#[cfg(feature = "station-rf")]
        if special.station_type == station::StationType::RadioFrequency as u8 {
            if let Ok(data) = <station::RFStationData as station::TryFromLegacyString>::try_from_legacy_string(&special.data) {
                special_data = station::SpecialStationData::RF(data);
            } else {
                return Ok(error::ReturnErrorCode::DataOutOfBound);
            }
        }

        if special.station_type == station::StationType::Remote as u8 {
            if let Ok(data) = <station::RemoteStationData as station::TryFromLegacyString>::try_from_legacy_string(&special.data) {
                special_data = station::SpecialStationData::REMOTE(data);
            } else {
                return Ok(error::ReturnErrorCode::DataOutOfBound);
            }
        }

        //#[cfg(feature = "station-gpio")]
        if special.station_type == station::StationType::GPIO as u8 {
            if let Ok(data) = <station::GPIOStationData as station::TryFromLegacyString>::try_from_legacy_string(&special.data) {
                // Check that the pin is not a "reserved" pin (used by firmware)
                if open_sprinkler.is_gpio_pin_used(&data.pin) {
                    return Ok(error::ReturnErrorCode::DataOutOfBound);
                }

                special_data = station::SpecialStationData::GPIO(data);
            } else {
                return Ok(error::ReturnErrorCode::DataOutOfBound);
            }
        }

        if special.station_type == station::StationType::HTTP as u8 {
            if let Ok(data) = <station::HTTPStationData as station::TryFromLegacyString>::try_from_legacy_string(&special.data) {
                special_data = station::SpecialStationData::HTTP(data);
            } else {
                return Ok(error::ReturnErrorCode::DataOutOfBound);
            }
        }

        open_sprinkler.config.stations[special.station_index].station_type = match special.station_type {
            0 => station::StationType::Standard,
            1 => station::StationType::RadioFrequency,
            2 => station::StationType::Remote,
            3 => station::StationType::GPIO,
            4 => station::StationType::HTTP,
            _ => station::StationType::Other,
        };
        open_sprinkler.config.stations[special.station_index].sped = Some(special_data.clone());
        tracing::debug!("Special Station: {:?}", special_data);
    }

    // master1
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'm', |station_index, value| {
        tracing::trace!("set use_master_1 @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.use_master[0] = value;
    });

    // master2
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'n', |station_index, value| {
        tracing::trace!("set use_master_2 @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.use_master[1] = value;
    });

    // ignore rain
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'i', |station_index, value| {
        tracing::trace!("set ignore_rain @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.ignore_rain_delay = value;
    });

    // ignore sensor1
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'j', |station_index, value| {
        tracing::trace!("set ignore_sensor_1 @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.ignore_sensor[0] = value;
    });

    // ignore sensor2
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'k', |station_index, value| {
        tracing::trace!("set ignore_sensor_2 @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.ignore_sensor[1] = value;
    });

    // sequential
    change_stations_attrib(open_sprinkler.get_board_count(), &parameters, 'q', |station_index, value| {
        tracing::trace!("set is_sequential @ station_{} = {}", station_index, value);
        open_sprinkler.config.stations[station_index].attrib.is_sequential = value;
    });

    open_sprinkler.config.write()?;

    Ok(error::ReturnErrorCode::Success)
}

fn change_stations_attrib<'a>(board_count: usize, parameters: &web::Query<HashMap<String, String>>, attrib: char, mut mutator: impl FnMut(usize, bool) + 'a) {
    for board_index in 0..board_count {
        if let Some(value) = parameters.get(&format!("{}{}", attrib, board_index)).and_then(|v| v.parse::<u8>().map(|v| Some(v)).unwrap_or(None)) {
            for line in 0..station::SHIFT_REGISTER_LINES {
                let station_index = board_index * station::SHIFT_REGISTER_LINES + line;

                (mutator)(station_index, (value & (0x01 << line)) != 0);
            }
        }
    }
}
