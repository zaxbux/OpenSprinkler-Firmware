use serde::Serialize;

use crate::opensprinkler::OpenSprinkler;

#[derive(Serialize)]
pub struct Payload {
    sn: Vec<u8>,
    nstations: usize,
}

impl Payload {
    pub fn new(open_sprinkler: &OpenSprinkler) -> Self {
        let nstations = open_sprinkler.get_station_count();

        let mut sn: Vec<u8> = Vec::with_capacity(nstations);

        for station_index in 0..nstations {
            sn.push(if open_sprinkler.state.station.active[station_index >> 3][station_index & 0x07] { 1 } else { 0 });
        }

        Self { sn, nstations }
    }
}