use serde::Serialize;

use crate::{
    opensprinkler::{program, Controller},
    server::legacy::values::programs::ProgramDataLegacy,
};

#[derive(Serialize)]
pub struct Payload {
    nprogs: usize,
    nboards: usize,
    mnp: usize,
    mnst: usize,
    pnsize: usize,
    #[serde(rename = "pd")]
    program_data: Vec<ProgramDataLegacy>,
}

impl Payload {
    pub fn new(open_sprinkler: &Controller) -> Self {
        let program_data = open_sprinkler.config.programs.iter().map(|prog| ProgramDataLegacy::from(prog)).collect();

        Self {
            nprogs: open_sprinkler.config.programs.len(),
            nboards: open_sprinkler.config.get_board_count(),
            mnp: program::MAX_NUM_PROGRAMS,
            mnst: program::MAX_NUM_START_TIMES,
            pnsize: program::PROGRAM_NAME_SIZE,
            program_data,
        }
    }
}
