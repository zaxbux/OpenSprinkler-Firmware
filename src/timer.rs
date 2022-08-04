#[cfg(all(feature = "raspberry_pi", unix))]
pub use ruspiro_timer::sleep;

#[cfg(not(feature = "raspberry_pi"))]
pub use std::thread::sleep;