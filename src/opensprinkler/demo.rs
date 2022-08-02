pub fn get_gpio_pin(pin: u8) -> rppal::gpio::Result<rppal::gpio::IoPin> {
	Err(rppal::gpio::Error::PinNotAvailable(pin))
}