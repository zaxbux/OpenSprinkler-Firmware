#![feature(proc_macro_hygiene, decl_macro)]

use rocket::response::status;

pub extern "C" fn string_remove_space(s: &mut String) {
    s.retain(|c| !c.is_whitespace());
}

pub extern "C" fn parse_listdata(p: *mut *mut c_char) -> u16
{
	char *pv;
	let i = 0;
	tmp_buffer[i] = 0;
	// copy to tmp_buffer until a non-number is encountered
	for (pv = (*p); pv < (*p) + 10; pv++)
	{
		if ((*pv) == '-' || (*pv) == '+' || ((*pv) >= '0' && (*pv) <= '9'))
			tmp_buffer[i++] = (*pv);
		else
			break;
	}
	tmp_buffer[i] = 0;
	*p = pv + 1;
	return (uint16_t)atol(tmp_buffer);
}

#[macro_use] extern crate rocket;

#[get("/")]
fn index() {
	format!("<!DOCTYPE html>\
		<html>\
			<head>\
				<meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0,minimum-scale=1.0,user-scalable=no\">\
				<meta name=\"firmware-version\" content=\"{}\">\
			</head>\
			<body>\
				<script src=\"{}/home.js\"></script>\
			</body>\
		</html>", OS_FIRMWARE_VERSION, SOPT_JAVASCRIPT_URL)
}

#[get("/su")]
fn server_view_script_url() {

}

#[get("/cv")]
fn server_change_values() -> &'static str {

}

#[get("/jc")]
fn server_json_controller() -> &'static str {

}

#[get("/dp")]
fn server_delete_program() -> &'static str {

}

#[get("/cp")]
fn server_change_program() -> &'static str {

}

#[get("/cr")]
fn server_change_runonce() -> &'static str {

}

#[get("/mp")]
fn server_manual_program() -> &'static str {

}

#[get("/up")]
fn server_moveup_program() -> &'static str {

}

#[get("/jp")]
fn server_json_programs() -> &'static str {

}

#[get("/co")]
fn server_change_options() -> &'static str {

}

#[get("/jo")]
fn server_json_options() -> &'static str {

}

#[get("/sp")]
fn server_change_password() -> &'static str {

}

#[get("/js")]
fn server_json_status() -> &'static str {

}

#[get("/cm")]
fn server_change_manual() -> &'static str {

}

#[get("/cs")]
fn server_change_stations() -> &'static str {

}

#[get("/jn")]
fn server_json_stations() -> &'static str {

}

#[get("/je")]
fn server_json_station_special() -> &'static str {

}

#[get("/jl?<type>&<hist>&<start>&<end>")]
fn server_json_log(r#type: Option<String>, hist: Option<u16>, start: Option<u64>, end: Option<u64>) -> &'static str {

}

/// Delete logs
/// 
/// # Arguments
/// * `day` - (epoch time) / 86400
#[get("/dl?<day>")]
fn server_delete_log(day: &RawStr) -> status::NoContent {
	delete_log(day);

	status::NoContent
}

#[get("/su")]
fn server_view_scripturl() -> &'static str {

}

#[get("/cu")]
fn server_change_scripturl() -> &'static str {

}

#[get("/ja")]
fn server_json_all() -> &'static str {

}
