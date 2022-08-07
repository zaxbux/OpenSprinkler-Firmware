use core::fmt;

pub trait WeatherAlgorithm {
	fn use_manual_scale(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manual;

impl WeatherAlgorithm for Manual {
	fn use_manual_scale(&self) -> bool {
		true
	}
}

impl fmt::Display for Manual {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Manual")
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct RainDelay;

impl WeatherAlgorithm for RainDelay {
	fn use_manual_scale(&self) -> bool {
		true
	}
}

impl fmt::Display for RainDelay {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Rain Delay")
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Zimmerman;

impl WeatherAlgorithm for Zimmerman {
	fn use_manual_scale(&self) -> bool {
		false
	}
}

impl fmt::Display for Zimmerman {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Zimmerman")
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evapotranspiration;

impl WeatherAlgorithm for Evapotranspiration {
	fn use_manual_scale(&self) -> bool {
		false
	}
}

impl fmt::Display for Evapotranspiration {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Evapotranspiration")
	}
}