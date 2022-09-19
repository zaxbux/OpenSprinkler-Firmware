use std::sync::{Arc, Mutex};

use actix_web::{web, Responder, Result};
use serde::Deserialize;

use crate::{
    opensprinkler::{config::RebootCause, OpenSprinkler},
    server::legacy::{self, error},
};

pub const RAIN_DELAY_HOURS_MAX: u16 = 32767;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ChangeVariablesRequest {
    /// Reset all stations (including those waiting to run). Value: `0` or `1`.
    #[serde(rename = "rsn", deserialize_with = "legacy::de::bool_from_int")]
    reset_stations: bool,
    /// Trigger a firmware update. Value: `0` or `1`.
    #[serde(rename = "update", deserialize_with = "legacy::de::bool_from_int")]
    update_fw: bool,
    /// Reboot the controller. Value: `0` or `1`.
    #[serde(rename = "rbt", deserialize_with = "legacy::de::bool_from_int")]
    reboot: bool,
    /// Set the controller as enabled/disabled. Value: `0` or `1`.
    #[serde(rename = "en", deserialize_with = "legacy::de::bool_from_int_option")]
    enable_controller: Option<bool>,
    /// Set remote extension mode as enabled/disabled. Value: `0` or `1`.
    #[serde(rename = "re", deserialize_with = "legacy::de::bool_from_int_option")]
    enable_remote_ext: Option<bool>,
    /// Set rain delay time (in hours). Range is `0` to [RAIN_DELAY_HOURS_MAX]. A value of **0** turns off rain delay.
    #[serde(rename = "rd")]
    rain_delay: Option<u16>,
}

impl Default for ChangeVariablesRequest {
    fn default() -> Self {
        Self {
            reset_stations: false,
            update_fw: false,
            reboot: false,
            enable_controller: None,
            enable_remote_ext: None,
            rain_delay: None,
        }
    }
}

/// URI: `/cv`
pub async fn handler(open_sprinkler: web::Data<Arc<Mutex<OpenSprinkler>>>, parameters: web::Query<ChangeVariablesRequest>) -> Result<impl Responder> {
    let mut open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    if parameters.reset_stations {
        open_sprinkler.state.program.queue.reset_all_stations();
    }

    if parameters.update_fw {
        open_sprinkler.update_dev()?;
    }

    if parameters.reboot {
        open_sprinkler.reboot_dev(RebootCause::Web)?;
    }

    if let Some(enable_controller) = parameters.enable_controller {
        if enable_controller {
            open_sprinkler.enable()?;
        } else {
            open_sprinkler.disable()?;
        }
    }

    if let Some(rain_delay) = parameters.rain_delay {
        if rain_delay == 0 {
            open_sprinkler.rain_delay_stop();
        } else if rain_delay > 0 && rain_delay <= RAIN_DELAY_HOURS_MAX {
            open_sprinkler.rain_delay_start(chrono::Utc::now() + chrono::Duration::hours(rain_delay.into()));
        } else {
            return Ok(error::ReturnErrorCode::DataOutOfBound);
        }
    }

    if let Some(enable_remote_ext_mode) = parameters.enable_remote_ext {
        open_sprinkler.config.enable_remote_ext_mode = enable_remote_ext_mode;
        open_sprinkler.config.write()?;
    }

    Ok(error::ReturnErrorCode::Success)
}
