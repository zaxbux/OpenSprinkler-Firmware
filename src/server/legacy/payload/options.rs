use serde::Serialize;

use crate::{
    opensprinkler::{sensor, station, Controller},
    server::legacy::IntoLegacyFormat,
};

#[derive(Debug, Serialize)]
pub struct Payload {
    tz: u8,
    mexp: u8,
    ext: u8,
    sdt: u8,
    mas: u8,
    mton: u8,
    mtof: u8,
    wl: u8,
    den: u8,
    uwt: u8,
    lg: u8,
    mas2: u8,
    mton2: u8,
    mtof2: u8,
    fpr0: u8,
    fpr1: u8,
    re: u8,
    sar: u8,
    ife: u8,
    sn1t: u8,
    sn1o: u8,
    sn1on: u8,
    sn1of: u8,
    sn2t: u8,
    sn2o: u8,
    sn2on: u8,
    sn2of: u8,

    fwv: u8,
    fwm: u8,
    devid: (),
    ipas: u8,
    hp0: u8,
    hp1: u8,
    hwv: u8,
    hwt: u8,
    dexp: i8,
    reset: u8,
}

impl Payload {
    pub fn new(open_sprinkler: &Controller) -> Self {
        let web_port: u16 = 0;

        // Convert [u16] HTTP port to legacy encoded format
        let hp0 = web_port as u8 & 0xff;
        let hp1 = (web_port >> 8) as u8 & 0xff;

        // Convert [u16] flow pulse rate to legacy encoded format
        let fpr = open_sprinkler.config.flow_pulse_rate;
        let fpr0 = (fpr * 100) as u8 & 0xff;
        let fpr1 = ((fpr * 100) >> 8) as u8 & 0xff;

        Self {
            tz: open_sprinkler.config.timezone,
            mexp: station::MAX_EXT_BOARDS as u8,
            ext: open_sprinkler.config.extension_board_count as u8,
            sdt: open_sprinkler.config.station_delay_time,
            mas: open_sprinkler.config.master_stations[0].station.and_then(|i| Some(i + 1)).unwrap_or(0) as u8,
            mton: open_sprinkler.config.master_stations[0].get_adjusted_on_time() as u8,
            mtof: open_sprinkler.config.master_stations[0].get_adjusted_off_time() as u8,
            wl: (open_sprinkler.config.get_water_scale() * 100.0) as u8,
            den: if open_sprinkler.config.enable_controller { 1 } else { 0 },
            uwt: open_sprinkler.config.weather.algorithm.as_ref().and_then(|algo| Some(algo.get_id() as u8)).unwrap_or_else(|| 0),
            lg: if open_sprinkler.config.enable_log { 1 } else { 0 },
            mas2: open_sprinkler.config.master_stations[1].station.and_then(|i| Some(i + 1)).unwrap_or(0) as u8,
            mton2: open_sprinkler.config.master_stations[1].get_adjusted_on_time() as u8,
            mtof2: open_sprinkler.config.master_stations[1].get_adjusted_off_time() as u8,
            fpr0,
            fpr1,
            re: if open_sprinkler.config.enable_remote_ext_mode { 1 } else { 0 },
            sar: if open_sprinkler.config.enable_special_stn_refresh { 1 } else { 0 },
            ife: open_sprinkler.config.ifttt.events.into_legacy_format(),
            sn1t: open_sprinkler.config.sensors[0].sensor_type.unwrap_or(sensor::SensorType::None) as u8,
            sn1o: open_sprinkler.config.sensors[0].normal_state as u8,
            sn1on: open_sprinkler.config.sensors[0].delay_on,
            sn1of: open_sprinkler.config.sensors[0].delay_off,
            sn2t: open_sprinkler.config.sensors[1].sensor_type.unwrap_or(sensor::SensorType::None) as u8,
            sn2o: open_sprinkler.config.sensors[1].normal_state as u8,
            sn2on: open_sprinkler.config.sensors[1].delay_on,
            sn2of: open_sprinkler.config.sensors[1].delay_off,

            // region: Deprecated options
            fwv: 219, // Last version of the legacy firmware supported
            fwm: 9,
            devid: (), // Device ID is not used on RPi
            ipas: 0,   // Authentication is required
            hp0,       // Read-only, only for compatibility
            hp1,       // Read-only, only for compatibility
            hwv: 0x40, // Hardware version "base" for RPi
            hwt: 0xFF, // Unknown HW type
            dexp: -1,  // RPi version lacks detection circuitry
            reset: 0,
            // endregion: Deprecated options
        }
    }
}
