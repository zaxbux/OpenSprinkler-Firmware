use std::net::IpAddr;

use serde::Serialize;

use crate::{
    opensprinkler::{program, weather, OpenSprinkler, FLOW_COUNT_REALTIME_WINDOW},
    server::legacy::{utils, values::options::MqttConfigJson},
};

#[derive(Serialize)]
pub struct Payload {
    devt: i64,
    nbrd: usize,
    en: u8,
    sn1: u8,
    sn2: u8,
    rd: u8,
    rdst: i64,
    sunrise: u16,
    sunset: u16,
    eip: Option<IpAddr>,
    lwc: i64,
    lswc: i64,
    lupt: i64,
    lrbtc: u8,
    lrun: [i64; 4],
    mac: Option<String>,
    loc: String,
    jsp: String,
    wsp: String,
    wto: String,
    ifkey: String,
    mqtt: MqttConfigJson,
    wtdata: weather::WeatherServiceRawData,
    wterr: i8,
    flcrt: i32,
    flwrt: i64,
    sbits: Vec<u8>,
    ps: Vec<[i64; 3]>,
}

impl Payload {
    pub fn new(open_sprinkler: &OpenSprinkler) -> Self {
        let curr_time = chrono::Utc::now().timestamp();

        let station_count = open_sprinkler.get_station_count();

        let mut ps = Vec::with_capacity(station_count);

        for station_index in 0..station_count {
            if let Some(qid) = open_sprinkler.state.program.queue.station_qid[station_index] {
                if let Some(q) = open_sprinkler.state.program.queue.queue.get(qid) {
                    let mut rem = if curr_time >= q.start_time { q.start_time + q.water_time - curr_time } else { q.water_time };

                    if rem > 65535 {
                        rem = 0;
                    }

                    // @todo: verify against legacy implementation
                    let program_index = match q.program_index {
                        Some(i) => i as i64,
                        None => match q.program_start_type {
                            program::ProgramStartType::Test => 99,
                            program::ProgramStartType::TestShort => 99,
                            program::ProgramStartType::RunOnce => 254,
                            program::ProgramStartType::User => 99,
                        },
                    };

                    ps.push([program_index + 1, rem, q.start_time]);
                }
            } else {
                ps.push([0, 0, 0]);
            }
        }

        let mut sbits = Vec::with_capacity(open_sprinkler.get_board_count());

        for board_state in open_sprinkler.state.station.active {
            let mut b = 0;
            for (i, s) in board_state.iter().enumerate() {
                b = b & (if *s { 1 } else { 0 } << i);
            }
            sbits.push(b);
        }

        Self {
            devt: open_sprinkler.now_tz_seconds(),
            nbrd: open_sprinkler.config.extension_board_count + 1,
            en: utils::bool_to_u8(open_sprinkler.config.enable_controller),
            sn1: utils::bool_to_u8(open_sprinkler.state.sensor.state(0)),
            sn2: utils::bool_to_u8(open_sprinkler.state.sensor.state(0)),
            rd: utils::bool_to_u8(open_sprinkler.state.rain_delay.active_now),
            rdst: open_sprinkler.config.rain_delay_stop_time.unwrap_or(-1),
            sunrise: open_sprinkler.config.sunrise_time,
            sunset: open_sprinkler.config.sunset_time,
            eip: open_sprinkler.state.external_ip,
            lwc: open_sprinkler.state.weather.checkwt_lasttime.unwrap_or(-1),
            lswc: open_sprinkler.state.weather.checkwt_success_lasttime.unwrap_or(-1),
            lupt: open_sprinkler.state.reboot_timestamp,
            lrbtc: open_sprinkler.config.reboot_cause as u8,
            lrun: if let Some(ref last_run) = open_sprinkler.state.program.queue.last_run {
                [
                    last_run.station_index as i64,
                    last_run.program_index.unwrap_or(99) as i64 + 1, // @fixme program index for non-user/scheduled programs
                    last_run.duration.map(|dur| dur.num_seconds()).unwrap_or(0),
                    last_run.end_time.unwrap(),
                ]
            } else {
                [0, 0, 0, 0]
            },
            mac: if let Some(mac) = open_sprinkler.get_hw_mac() { Some(mac.to_string()) } else { None },
            loc: open_sprinkler.config.location.to_string(),
            jsp: open_sprinkler.config.js_url.clone(),
            wsp: open_sprinkler.config.weather.service_url.replace("https://", "").replace("http://", ""),
            wto: open_sprinkler.config.weather.options.clone().unwrap_or_else(|| String::from("{}")),
            ifkey: open_sprinkler.config.ifttt.web_hooks_key.clone(),
            mqtt: MqttConfigJson::from(open_sprinkler.config.mqtt.clone()),
            wtdata: open_sprinkler.state.weather.raw_data.clone(),
            wterr: open_sprinkler.state.weather.last_response_code.clone().unwrap_or(weather::ErrorCode::Unknown(-1)).into(),
            flcrt: open_sprinkler.state.flow.count_realtime_now,
            flwrt: FLOW_COUNT_REALTIME_WINDOW,
            sbits,
            ps,
        }
    }
}
