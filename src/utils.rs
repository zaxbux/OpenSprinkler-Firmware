use std::{ffi::{CStr, CString}, env, path::PathBuf, cmp::{min, max}};

use libc::{c_char};

pub fn get_str_from_cstr(ptr: *const c_char) -> &'static str {
	let c_str = unsafe { CStr::from_ptr(ptr) };
	c_str.to_str().unwrap()
}

#[no_mangle]
pub extern "C" fn get_runtime_path() -> *const c_char {
	CString::new(env::current_dir().unwrap().to_str().unwrap()).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn get_filename_fullpath(fname: *const c_char) -> *const c_char {
	let c_str = unsafe { CStr::from_ptr(fname) };
	let path = PathBuf::from(c_str.to_str().unwrap()).canonicalize();
	if path.is_err() {
		let mut path = env::current_dir().unwrap();
		path.push(c_str.to_str().unwrap());
		return CString::new(path.to_str().unwrap()).unwrap().into_raw();
	}
	CString::new(path.unwrap().to_str().unwrap()).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn file_exists(fname: *const c_char) -> bool {
	PathBuf::from(get_str_from_cstr(fname)).exists()
}

/// encode a 16-bit signed water time (-600 to 600) to unsigned unsigned char (0 to 240)
#[no_mangle]
pub extern "C" fn water_time_encode_signed(i: i16) -> u8
{
	((max(min(i, 600), -600) + 600) / 5) as u8
}

/// decode a 8-bit unsigned unsigned char (0 to 240) to a 16-bit signed water time (-600 to 600)
#[no_mangle]
pub extern "C" fn water_time_decode_signed(i: u8) -> i16
{
	(min(i as i16, 240) - 120) * 5
}

// pub fn get_firmware_version() -> u64 {
// 	let str_ver = format!("{}{}{}", env!("CARGO_PKG_VERSION_MAJOR"), env!("CARGO_PKG_VERSION_MINOR"), env!("CARGO_PKG_VERSION_PATCH"));

// 	str_ver.parse().unwrap()
// }

// pub fn get_firmware_version_pre() -> u64 {
// 	env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap()
//}