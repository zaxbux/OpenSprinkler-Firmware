use crate::opensprinkler::OpenSprinkler;

pub fn list(open_sprinkler: &OpenSprinkler) {
	let mut config = open_sprinkler.config.clone();
	config.stations = Vec::new();
	println!("Config: {}", config);
}

pub fn set(config_set: Vec<String>, open_sprinkler: &mut OpenSprinkler) -> Result<(), &'static str>{
	let [name, value] = [&config_set[0], &config_set[1]];

	match name.as_str() {
		"weather.algorithm" => {
			open_sprinkler.config.weather.set_algorithm(Some(value.parse().expect("Could not parse ID")));
			println!("Set weather.algorithm: {:?}", open_sprinkler.config.weather.algorithm);
			Ok(())
		},
		"mqtt.enabled" => {
			open_sprinkler.config.mqtt.enabled = match value.parse::<i32>().unwrap_or(0) {
				0 => false,
				1 => true,
				_ => false,
			};
			println!("Set mqtt.enabled: {:?}", open_sprinkler.config.mqtt.enabled);
			Ok(())
		},
		&_ => Err("Unknown config key"),
	}
}

pub fn reset(open_sprinkler: &OpenSprinkler) {
	open_sprinkler.config.write_default().unwrap();
	println!("Reset controller to defaults");
}