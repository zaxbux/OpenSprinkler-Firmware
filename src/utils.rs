use std::cmp::{max, min};

/// encode a 16-bit signed water time (-600 to 600) to unsigned unsigned char (0 to 240)
pub fn water_time_encode_signed(signed: i16) -> u8 {
    u8::try_from(max(min(signed, 600), -600) + 600).unwrap() / 5
}

/// decode a 8-bit unsigned unsigned char (0 to 240) to a 16-bit signed water time (-600 to 600)
pub fn water_time_decode_signed(unsigned: u8) -> i16 {
    (min(i16::from(unsigned), 240) - 120) * 5
}

/// Formats a duration (in seconds) into a string HH:mm:ss
pub fn duration_to_hms<T: Into<i64>>(duration: T) -> String {
    let duration: i64 = duration.into();
    let h = duration / 3600;
    let m = (duration / 60) - (h * 60);
    let s = duration % 60;
    format!("{:.0}:{:02.0}:{:02.0}", h, m, s)
}

const MAGIC_SUNRISE_TO_SUNSET: u16 = 65534;
const MAGIC_SUNSET_TO_SUNRISE: u16 = 65535;

/// Resolves water time.
/// 
/// Returns the watering time as an [i64].
/// 
/// ## Arguments
/// 
/// - `water_time`: A program's watering time (seconds).
/// - `sunrise_time`: The current sunrise offset (minutes).
/// - `sunset_time`: The current sunset offset (minutes).
/// 
/// If the value is one of the following *MAGIC_** numbers, the duration (in seconds) between sunrise/sunset.
/// 
/// - [MAGIC_SUNRISE_TO_SUNSET]
/// - [MAGIC_SUNSET_TO_SUNRISE]
/// 
/// The maximum runtime of a station is 18 hours or 64800 seconds, this value will fit in a [u16].
pub fn water_time_resolve(water_time: u16, sunrise_time: u16, sunset_time: u16) -> f32 {
    match water_time {
        MAGIC_SUNRISE_TO_SUNSET => ((sunset_time - sunrise_time) * 60).into(),
        MAGIC_SUNSET_TO_SUNRISE => ((sunrise_time + 1440 - sunset_time) * 60).into(), // 1440 minutes = 24 hours
        _ => water_time.into(),
    }
}
