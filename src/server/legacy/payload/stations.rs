use serde::Serialize;

use crate::opensprinkler::{station, Controller};

const STATION_NAME_MAX_LEN: u8 = 32;

#[derive(Serialize)]
pub struct Payload {
    maxlen: u8,
    snames: Vec<String>,
    masop: Vec<u8>,
    masop2: Vec<u8>,
    ignore_rain: Vec<u8>,
    ignore_sn1: Vec<u8>,
    ignore_sn2: Vec<u8>,
    stn_dis: Vec<u8>,
    stn_seq: Vec<u8>,
    stn_spe: Vec<u8>,
}

impl Payload {
    pub fn new(open_sprinkler: &Controller) -> Self {
        let station_count = open_sprinkler.config.get_station_count();

        let mut snames = Vec::<String>::with_capacity(station_count);
        let mut masop = vec![0; station_count];
        let mut masop2 = vec![0; station_count];
        let mut ignore_rain = vec![0; station_count];
        let mut ignore_sn1 = vec![0; station_count];
        let mut ignore_sn2 = vec![0; station_count];
        let mut stn_dis = vec![0; station_count];
        let mut stn_seq = vec![0; station_count];
        let mut stn_spe = vec![0; station_count];

        for station_index in 0..station_count {
            if let Some(station) = open_sprinkler.config.stations.get(station_index) {
                snames.push(station.name.clone());

                let board_index = station_index >> 3;
                let line = station_index & 0x07;

                if station.attrib.use_master[0] {
                    masop[board_index] += 1 << line;
                }

                if station.attrib.use_master[1] {
                    masop2[board_index] += 1 << line;
                }

                if station.attrib.ignore_rain_delay {
                    ignore_rain[board_index] += 1 << line;
                }

                if station.attrib.ignore_sensor[0] {
                    ignore_sn1[board_index] += 1 << line;
                }

                if station.attrib.ignore_sensor[1] {
                    ignore_sn2[board_index] += 1 << line;
                }

                if station.attrib.is_disabled {
                    stn_dis[board_index] += 1 << line;
                }

                if station.attrib.is_sequential {
                    stn_seq[board_index] += 1 << line;
                }

                if station.station_type != station::StationType::Standard {
                    stn_spe[board_index] += 1 << line;
                }
            }
        }

        Self {
            maxlen: STATION_NAME_MAX_LEN,
            snames,
            masop,
            masop2,
            ignore_rain,
            ignore_sn1,
            ignore_sn2,
            stn_dis,
            stn_seq,
            stn_spe,
        }
    }
}
