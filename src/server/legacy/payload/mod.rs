mod options;
mod programs;
mod settings;
mod stations;
mod status;

pub use options::Payload as Options;
pub use programs::Payload as Programs;
use serde::Serialize;
pub use settings::Payload as Settings;
pub use stations::Payload as Stations;
pub use status::Payload as Status;

use crate::opensprinkler::OpenSprinkler;

#[derive(Serialize)]
pub struct All {
    settings: Settings,
    options: Options,
    stations: Stations,
    status: Status,
    programs: Programs,
}

impl All {
    pub fn new(open_sprinkler: &OpenSprinkler) -> Self {
        Self {
            settings: Settings::new(open_sprinkler),
            options: Options::new(open_sprinkler),
            stations: Stations::new(open_sprinkler),
            status: Status::new(open_sprinkler),
            programs: Programs::new(open_sprinkler),
        }
    }
}