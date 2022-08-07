use crate::opensprinkler::OpenSprinkler;

pub fn list(open_sprinkler: &OpenSprinkler) {
	let mut config = open_sprinkler.controller_config.clone();
	config.stations = Vec::new();
	let json = serde_json::to_string_pretty(&config);
	println!("Config: {}", json.unwrap());
}

pub fn set(config_set: Vec<String>, open_sprinkler: &mut OpenSprinkler) -> Result<(), &'static str>{
	let [name, value] = [&config_set[0], &config_set[1]];

	match name.as_str() {
		"weather.algorithm" => {
			open_sprinkler.controller_config.weather.set_algorithm(Some(value.parse().expect("Could not parse ID")));
			println!("Set weather.algorithm: {:?}", open_sprinkler.controller_config.weather.algorithm);
			Ok(())
		},
		&_ => Err("Unknown config key"),
	}
}

pub fn reset(open_sprinkler: &OpenSprinkler) {
	open_sprinkler.config.commit_defaults();
	println!("Reset controller to defaults");
}