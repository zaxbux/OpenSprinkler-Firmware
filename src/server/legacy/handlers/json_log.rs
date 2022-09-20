use std::{
    fs,
    io::{self, BufRead},
    sync,
};

use actix_web::{web, Error, HttpRequest, HttpResponse, Responder, Result};

use serde::Deserialize;

use crate::{opensprinkler::Controller, server::legacy::error};

#[derive(Deserialize)]
#[serde(default)]
pub struct JsonLogRequest {
    /// Filter log record type
    ///
    /// Can be one of: `s1`, `rd`, `wl`, `fl`, `s2`, `cu`
    ///
    /// If not provided, all records will be returned (excluding "wl" and "fl")
    #[serde(rename = "type")]
    filter_type: Option<String>,
    /// History (past *N* days)
    ///
    /// If set, `start` and `end` are ignored.
    ///
    /// Range: \[`0`, `365`\]
    #[serde(rename = "hist")]
    days: Option<u64>,
    /// Start (UTC timestamp)
    ///
    /// Unit: seconds
    start: Option<u64>,
    /// End (UTC timestamp)
    ///
    /// Unit: seconds
    ///
    /// Must be a timestamp greater than `start`.
    end: Option<u64>,
}

impl Default for JsonLogRequest {
    fn default() -> Self {
        Self {
            filter_type: None,
            days: None,
            start: None,
            end: None,
        }
    }
}

/// URI: `/jl`
///
///
pub async fn handler(open_sprinkler: web::Data<sync::Arc<sync::Mutex<Controller>>>, parameters: web::Query<JsonLogRequest>, req: HttpRequest) -> Result<impl Responder> {
    let open_sprinkler = open_sprinkler.lock().map_err(|_| error::InternalError::SyncError)?;

    let (start, end) = if let Some(days) = parameters.days {
        if days > 365 {
            return Ok(error::ReturnErrorCode::DataOutOfBound.respond_to(&req));
        }
        let end = chrono::Utc::now().timestamp() as u64 / 86400;
        let start = end - days;
        (start, end)
    } else if let (Some(start), Some(end)) = (parameters.start, parameters.end) {
        (start / 84600, end / 86400)
    } else {
        return Ok(error::ReturnErrorCode::DataMissing.respond_to(&req));
    };

    // end must be greater than start, and the difference cannot be greater than 365 days
    if start > end || end - start > 365 {
        return Ok(error::ReturnErrorCode::DataOutOfBound.respond_to(&req));
    }

    let stream_log = async_stream::stream! {
        let mut bytes = web::BytesMut::new();

        bytes.extend_from_slice("[".as_bytes());
        let byte = bytes.split().freeze();
        yield Ok::<web::Bytes, Error>(byte);

        for i in start..end {
            let file = fs::OpenOptions::new().read(true).open(format!("logs/{}.txt", i))?;
            let reader = io::BufReader::new(file);

            if i > 0 {
                bytes.extend_from_slice(",".as_bytes());
            }

            for line in reader.lines() {
                let line = line.as_ref().unwrap();
                let parts: Vec<&str> = line.splitn(3, ',').collect();
                let line_type = parts[1].trim_matches(|c| c == '"');

                if let Some(ref filter_type) = parameters.filter_type {
                    if line_type != filter_type {
                        continue;
                    }
                } else {
                    if line_type == "wl" || line_type == "fl" {
                        continue;
                    }
                }

                bytes.extend_from_slice(line.as_bytes());
                bytes.extend_from_slice("\n".as_bytes());

                let byte = bytes.split().freeze();
                yield Ok::<web::Bytes, Error>(byte)
            }



            /* match serde_json::to_string(&task) {
                Ok(task) => {
                    bytes.extend_from_slice(task.as_bytes());
                    let byte = bytes.split().freeze();
                    yield Ok::<web::Bytes, Error>(byte)
                },
                Err(err) => error!("Tasks list stream error: {}", err)
            } */
        }

        bytes.extend_from_slice("]".as_bytes());
        let byte = bytes.split().freeze();
        yield Ok::<web::Bytes, Error>(byte);
    };

    Ok(HttpResponse::Ok().content_type("application/json").streaming(Box::pin(stream_log)))
}
