pub mod ser {
    use serde::Serializer;

    pub fn int_from_bool<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let v = match value {
            false => 0,
            true => 1,
        };

        serializer.serialize_u8(v)
    }
}

pub mod de {
    use serde::{
        de::{self, Unexpected},
        Deserialize, Deserializer,
    };

    pub fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(serde::de::Error::invalid_value(Unexpected::Unsigned(other as u64), &"0 or 1")),
        }
    }

    pub fn bool_from_int_option<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<u8>::deserialize(deserializer)? {
            Some(0) => Ok(Some(false)),
            Some(1) => Ok(Some(true)),
            other => Err(serde::de::Error::invalid_value(Unexpected::Unsigned(other.unwrap() as u64), &"0 or 1")),
        }
    }

    pub fn bool_from_string<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_ref() {
            "0" => Ok(false),
            "1" => Ok(true),
            other => Err(de::Error::invalid_value(Unexpected::Str(other), &"'0' or '1'")),
        }
    }

    pub fn int_array_from_string<'de, D>(deserializer: D) -> Result<Vec<u16>, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            // Remove outer brackets
            .trim_matches(|c| c == '[' || c == ']')
            // Split on comma
            .split(',')
            // Parse each element as u16 (with error handling)
            .map(|t| t.trim().parse().map_err(|_| serde::de::Error::invalid_value(Unexpected::Str(t), &"u16")))
            // Collect into Vec<u16>
            .collect()
    }
}

#[cfg(test)]
    mod tests {
        use actix_web::web;

        use crate::server::legacy::handlers::change_run_once::ChangeRunOnceRequest;
        use crate::server::legacy::handlers::change_program::ChangeProgramRequest;

        #[test]
        fn int_array_from_string_1() {
            let query = web::Query::<ChangeRunOnceRequest>::from_query("t=[65535]").unwrap();
            assert_eq!(query.times.len(), 1, "testing length of vector");
            assert_eq!(query.times[0], 65535, "testing first value");

            let query = web::Query::<ChangeRunOnceRequest>::from_query("t=[0,1,900]").unwrap();
            assert_eq!(query.times.len(), 3, "testing length of vector");
            assert_eq!(query.times[0], 0, "testing first value");
            assert_eq!(query.times[1], 1, "testing second value");
            assert_eq!(query.times[2], 900, "testing third value");
        }

        #[test]
        #[should_panic(expected = "Deserialize(Error(\"invalid value: string \\\"-1\\\", expected u16\"))")]
        fn int_array_from_string_2() {
            web::Query::<ChangeRunOnceRequest>::from_query("t=[-1]").unwrap();
        }

        #[test]
        #[should_panic(expected = "Deserialize(Error(\"invalid value: string \\\"x\\\", expected u16\"))")]
        fn int_array_from_string_3() {
            web::Query::<ChangeRunOnceRequest>::from_query("t=[x]").unwrap();
        }

        #[test]
        fn program_data_from_json_1() {
            let query = web::Query::<ChangeProgramRequest>::from_query("pid=0&v=[0,0,0,[0,0,0,0],[60,60,60,60]]&name=My%20Program").unwrap();
            println!("{:?}", query);
        }
    }
