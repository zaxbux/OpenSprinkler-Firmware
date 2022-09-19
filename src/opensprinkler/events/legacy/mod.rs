//! # Legacy event log
//! 
//! ## Record Format
//! 
//! ### Station
//! 
//! Format: \[ `program_index`\, `station_index`\, `duration`\, `end` \ (, `flow_rate`)]
//! 
//! 
//! * **program_index** starts at `1`
//! * **station_index** starts at `0`
//! * **duration** is in seconds
//! * **end** is the UTC timestamp (seconds)
//! * **flow_rate** is the flow volume in L/m (for all stations). It is only included when the first sensor is configured as the flow sensor.
//! 
//! ### Flow Sensor
//! 
//! Format: \[ `flow_count`\, "fl"\, `duration`\, `end` \]
//! 
//! * **flow_count** is the number of pulses for the duration.
//! 
//! ### Water Level
//! 
//! Format: \[ 0\, "wl"\, `water_level`\, `timestamp` \]
//! 
//! * **water_level** is the water scaling factor (%) at the logged timestamp.
//! 
//! ### Sensor / Rain Delay
//! 
//! Format: \[ 0\, `type`\, `duration`\, `end` \]
//! 
//! * **type** is: `s1`, `s2`, or `rd`
//! * **duration** is the duration that the sensor or rain delay was active.

pub mod writer;

#[derive(Debug)]
pub enum Error {
    /// The event does not have all of the necessary data.
    Incomplete,

    /// The event does not have the ability to be turned into an event log.
    NotImplemented,
}

pub trait LogEvent {
    fn try_into_bytes(&self) -> Result<Vec<u8>, Error> {
        Err(Error::NotImplemented)
    }
}

impl LogEvent for super::ProgramStartEvent {}
impl LogEvent for super::IpAddrChangeEvent {}
impl LogEvent for super::RebootEvent {}

impl LogEvent for super::StationEvent {
    fn try_into_bytes(&self) -> Result<Vec<u8>, Error> {
        if let (Some(program_index), Some(duration), Some(end)) = (self.program_index, self.duration, self.end_time) {
            if let Some(flow_volume) = self.flow_volume {
                return Ok(format!("[{},{},{},{},{:02.5}]", program_index + 1, self.station_index, duration.num_seconds(), end, flow_volume).into_bytes());
            } else {
                return Ok(format!("[{},{},{},{}]", program_index + 1, self.station_index, duration.num_seconds(), end).into_bytes());
            }
        }
        
        Err(Error::Incomplete)
    }
}

impl LogEvent for super::FlowSensorEvent {
    fn try_into_bytes(&self) -> Result<Vec<u8>, Error> {
        if let Some(duration) = self.duration {
            return Ok(format!("[{},\"fl\",{},{}]", self.count, duration.num_seconds(), self.end.timestamp()).into_bytes());
        }

        Err(Error::Incomplete)
    }
}

impl LogEvent for super::WaterScaleChangeEvent {
    fn try_into_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(format!("[0,\"wl\",{},{}]", (self.scale * 100.0) as u8, self.timestamp.timestamp()).into_bytes())
    }
}

impl LogEvent for super::BinarySensorEvent {
    fn try_into_bytes(&self) -> Result<Vec<u8>, Error> {
        if let Some(duration) = self.duration {
            return Ok(format!("[0,\"s{}\",{},{}]", self.index + 1, duration.num_seconds(), self.timestamp.timestamp()).into_bytes());
        }

        Err(Error::Incomplete)
    }
}

impl LogEvent for super::RainDelayEvent {
    fn try_into_bytes(&self) -> Result<Vec<u8>, Error> {
        if let Some(duration) = self.duration {
            return Ok(format!("[0,\"rd\",{},{}]", duration.num_seconds(), self.timestamp.timestamp()).into_bytes());
        }

        Err(Error::Incomplete)
    }
}

#[cfg(test)]
mod tests {
    use crate::opensprinkler::{events, program};

    use super::LogEvent;

    #[test]
    fn station_event() {
        let event = events::StationEvent::new(false, 0, "")
        .end_time(6800)
                            .duration(1234)
                            .program_index(Some(1))
                            .program_type(program::ProgramStartType::User);
        
        assert_eq!(LogEvent::try_into_bytes(&event).unwrap(), String::from("[2,0,1234,68000]").into_bytes());
    }

    #[test]
    fn station_event_flow() {
        let event = events::StationEvent::new(false, 0, "")
        .end_time(68000)
                            .duration(1234)
                            .flow_volume(Some(20.678901))
                            .program_index(Some(1))
                            .program_type(program::ProgramStartType::User);
        
        assert_eq!(LogEvent::try_into_bytes(&event).unwrap(), String::from("[2,0,1234,68000,20.67890]").into_bytes());
    }

    #[test]
    fn weather_event() {
        let timestamp_now = chrono::DateTime::from_utc(chrono::NaiveDateTime::from_timestamp(68000, 0), chrono::Utc);

        let event = events::WaterScaleChangeEvent::new(0.5, timestamp_now);
        
        assert_eq!(LogEvent::try_into_bytes(&event).unwrap(), String::from("[0,\"wl\",50,68000]").into_bytes());
    }

    #[test]
    fn sensor_event() {
        let timestamp_now = 68000;

        let event = events::BinarySensorEvent::new(0, false, timestamp_now, Some(timestamp_now - 1234));
        
        assert_eq!(LogEvent::try_into_bytes(&event).unwrap(), String::from("[0,\"s1\",1234,68000]").into_bytes());
    }

    #[test]
    fn rain_delay_event() {
        let timestamp_now = 68000;

        let event = events::RainDelayEvent::new(false, timestamp_now, Some(timestamp_now - 1234));
        
        assert_eq!(LogEvent::try_into_bytes(&event).unwrap(), String::from("[0,\"rd\",1234,68000]").into_bytes());
    }
}