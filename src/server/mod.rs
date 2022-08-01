#![feature(proc_macro_hygiene, decl_macro)]

const IOPT_JSON_NAMES: [&str; IOPT_COUNT] = [
    "fwv", "tz", "hp0", "hp1", "hwv", "ext", "sdt", "mas", "mton", "mtof", "wl", "den", "con",
    "lit", "dim", "uwt", "lg", "mas2", "mton2", "mtof2", "fwm", "fpr0", "fpr1", "re", "sar", "ife",
    "sn1t", "sn1o", "sn2t", "sn2o", "sn1on", "sn1of", "sn2on", "sn2of", "reset",
];

enum IOPT_MAX {
    fwv = 0,
    tz = 108,
    hp0 = 255,
    hp1 = 255,
    hwv = 0,
    ext = MAX_EXT_BOARDS,
    sdt = 255,
    mas = MAX_NUM_STATIONS,
    mton = 255,
    mtof = 255,
    wl = 250,
    den = 1,
    con = 255,
    lit = 255,
    dim = 255,
    uwt = 255,
    lg = 1,
    mas2 = MAX_NUM_STATIONS,
    mton2 = 255,
    mtof2 = 255,
    fwm = 0,
    fpr0 = 255,
    fpr1 = 255,
    re = 1,
    sar = 1,
    ife = 255,
    sn1t = 255,
    sn1o = 1,
    sn2t = 255,
    sn2o = 1,
    sn1on = 255,
    sn1of = 255,
    sn2on = 255,
    sn2of = 255,
    reset = 1,
}

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
