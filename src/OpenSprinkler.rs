use libc::{c_uchar, time_t};
use rppal::gpio::Gpio;
//use utils::{get_firmware_version, get_firmware_version_pre};
use tracing::{error, info};

//use std::thread;
use std::io;
use std::os::unix::prelude::FileExt;
use std::{path::Path, time::SystemTime, fs::File};

mod system;
mod utils;
//mod mqtt;

/// Shift register **CLOCK** pin
const GPIO_SHIFT_REGISTER_CLOCK: u8 = 4;
/// Shift register **OE** (output enable) pin
const GPIO_SHIFT_REGISTER_OE: u8 = 17;
/// Shift register **LATCH** pin
const GPIO_SHIFT_REGISTER_LATCH: u8 = 22;
/// Shift register **DATA** pin
const GPIO_SHIFT_REGISTER_DATA: u8 = 27;
/// Sensor 1 pin
const GPIO_SENSOR_1: u8 = 14;
/// Sensor 2 pin
const GPIO_SENSOR_2: u8 = 23;
/// RF transmitter pin
const GPIO_RF_TX: u8 = 15;

/// scratch buffer size
const TMP_BUFFER_SIZE: usize = 255;

/// allow more zones for linux-based firmwares
const MAX_EXT_BOARDS: usize = 24;
/// maximum number of 8-zone boards including expanders
const MAX_NUM_BOARDS: usize = 1 + MAX_EXT_BOARDS;
/// maximum number of stations
const MAX_NUM_STATIONS: usize = MAX_NUM_BOARDS * 8;
/// maximum number of characters in each station name
const STATION_NAME_SIZE: usize = 32;
const STATION_SPECIAL_DATA_SIZE: usize = TMP_BUFFER_SIZE - STATION_NAME_SIZE - 12;

/// Number of integer options
/// HACK: Remove
const IOPT_COUNT: usize = 35;

/// maximum string option size
const MAX_SOPTS_SIZE: usize = 160;

#[repr(u8)]
enum HardwareVersionBase {
    #[deprecated(
        since = "3.0.0",
        note = "Rust port of firmware is not compatible with Arduino/ESP platforms"
    )]
    OpenSprinkler = 0x00,
    OpenSprinklerPi = 0x40,
    Simulated = 0xC0,
}

/// Non-volatile data structure
struct NVConData {
    /// Sunrise time (minutes)
    sunrise_time: u16,
    /// Sunset time (minutes)
    sunset_time: u16,
    /// Rain-delay stop time (seconds since unix epoch)
    rd_stop_time: u32,
    /// External IP @TODO: Add support for IPv6
    external_ip: u32,
    /// Reboot Cause
    /// (see [RebootCause])
    reboot_cause: RebootCause,
}

/// In the original C++ implementation, bitfields are used (for a total size of 4 bytes).
/// However, bitfields are not supported by rust. The `bitfield` crate could be a solution.
struct StationAttrib {
    mas: bool,
    igs: bool,
    mas2: bool,
    dis: bool,
    seq: bool,
    igs2: bool,
    igrd: bool,
    unused: bool,

    gid: u8, // 4 bits, plus 4 "dummy" bits
    //dummy: u8,
    reserved: [u8; 2],
}

struct StationData {
    name: [char; STATION_NAME_SIZE],
    attrib: StationAttrib,
    /// Station type
    r#type: u8,
    /// Special station data
    sped: [u8; STATION_SPECIAL_DATA_SIZE],
}

/// RF station data structures - Must fit in [STATION_SPECIAL_DATA_SIZE]
struct RFStationData {
    on: [u8; 6],
    off: [u8; 6],
    timing: [u8; 4],
}

/// Remote station data structures - Must fit in STATION_SPECIAL_DATA_SIZE
struct RemoteStationData {
    ip: [u8; 8],
    port: [u8; 4],
    sid: [u8; 2],
}

/// GPIO station data structures - Must fit in STATION_SPECIAL_DATA_SIZE
struct GPIOStationData {
    pin: [u8; 2],
    active: u8,
}

/// HTTP station data structures - Must fit in STATION_SPECIAL_DATA_SIZE
struct HTTPStationData {
    data: [char; STATION_SPECIAL_DATA_SIZE],
}

/// Volatile controller status bits
/// @TODO: Original implimentation was bitfield (5 bytes)
struct ConStatus {
    // operation enable (when set, controller operation is enabled)
    /* unsigned char enabled : 1;		*/
    // rain delay bit (when set, rain delay is applied)
    /* unsigned char rain_delayed : 1;	*/
    // sensor1 status bit (when set, sensor1 on is detected)
    /* unsigned char sensor1 : 1;		*/
    // HIGH means a program is being executed currently
    /* unsigned char program_busy : 1;	*/
    // HIGH means a safe reboot has been marked
    /* unsigned char safe_reboot : 1;	*/
    // number of network fails
    /* unsigned char network_fails : 3;*/
    // master station index
    /* unsigned char mas : 8;			*/
    // master2 station index
    /* unsigned char mas2 : 8;			*/
    // sensor2 status bit (when set, sensor2 on is detected)
    /* unsigned char sensor2 : 1;		*/
    // sensor1 active bit (when set, sensor1 is activated)
    /* unsigned char sensor1_active : 1;*/
    // sensor2 active bit (when set, sensor2 is activated)
    /* unsigned char sensor2_active : 1;*/
    // request mqtt restart
    /* unsigned char req_mqtt_restart : 1;*/
}

const IOPT_JSON_NAMES: [&str; IOPT_COUNT] = [
    "fwv", "tz", "hp0", "hp1", "hwv", "ext", "sdt", "mas", "mton", "mtof", "wl", "den", "con",
    "lit", "dim", "uwt", "lg", "mas2", "mton2", "mtof2", "fwm", "fpr0", "fpr1", "re", "sar", "ife",
    "sn1t", "sn1o", "sn2t", "sn2o", "sn1on", "sn1of", "sn2on", "sn2of", "reset",
];

const IOPT_MAX: [usize; IOPT_COUNT] = [
    0,
    108,
    255,
    255,
    0,
    MAX_EXT_BOARDS,
    255,
    MAX_NUM_STATIONS,
    255,
    255,
    250,
    1,
    255,
    255,
    255,
    255,
    1,
    MAX_NUM_STATIONS,
    255,
    255,
    0,
    255,
    255,
    1,
    1,
    255,
    255,
    1,
    255,
    1,
    255,
    255,
    255,
    255,
    1,
];

#[non_exhaustive]
struct DataFile;

impl DataFile {
    pub const INTEGER_OPTIONS: &'static str = "iopts.dat";
    pub const STRING_OPTIONS: &'static str = "sopts.dat";
    pub const STATIONS: &'static str = "stns.dat";
    pub const NV_CONTROLLER: &'static str = "nvcon.dat";
    pub const PROGRAMS: &'static str = "prog.dat";
    pub const DONE: &'static str = "done.dat";
}

#[repr(u8)]
enum RebootCause {
    None = 0,
    Reset = 1,
    Button = 2,
    #[deprecated(since = "3.0.0", note = "Wi-Fi is handled by OS")]
    ResetAP = 3,
    Timer = 4,
    Web = 5,
    #[deprecated(since = "3.0.0", note = "Wi-Fi is handled by OS")]
    WifiDone = 6,
    FirmwareUpdate = 7,
    WeatherFail = 8,
    NetworkFail = 9,
    #[deprecated(since = "3.0.0", note = "NTP is handled by OS")]
    NTP = 10,
    Program = 11,
    PowerOn = 99,
}

#[derive(PartialEq)]
enum StationType {
    /// Stnadard station
    Standard = 0x00,
    /// RF station
    RadioFrequency = 0x01,
    /// Remote OpenSprinkler station
    Remote = 0x02,
    /// GPIO station
    GPIO = 0x03,
    /// HTTP station
    HTTP = 0x04,
    /// Other station
    Other = 0xFF,
}

enum NotifyEvent {
    ProgramSched = 0x0001,
    Sensor1 = 0x0002,
    FlowSensor = 0x0004,
    WeatherUpdate = 0x0008,
    Reboot = 0x0010,
    StationOff = 0x0020,
    Sensor2 = 0x0040,
    RainDelay = 0x0080,
    StationOn = 0x0100,
}

enum SensorType {
    /// No sensor
    None = 0x00,
    /// Rain sensor
    Rain = 0x01,
    /// Flow sensor
    Flow = 0x02,
    /// Soil moisture sensor
    Soil = 0x03,
    /// Program switch sensor
    ProgramSwitch = 0xF0,
    /// Other sensor
    Other = 0xFF,
}

enum LogDataType {
    Station = 0x00,
    Sensor1 = 0x01,
    Raindelay = 0x02,
    Waterlevel = 0x03,
    Flowsense = 0x04,
    Sensor2 = 0x05,
    #[deprecated(
        since = "3.0.0",
        note = "OpenSprinkler Pi hardware does not include current sensing circuit"
    )]
    Current = 0x80,
}

/// Flow Count Window (sesonds)
///
/// For computing real-time flow rate.
const FLOW_COUNT_RT_WINDOW: u8 = 30;

const HARDWARE_VERSION: u8 = HardwareVersionBase::OpenSprinklerPi as u8;
// const FIRMWARE_VERSION: u64 = get_firmware_version();
// const FIRMWARE_VERSION_REVISION: u64 = get_firmware_version_pre();

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn rs_now_tz(tz: c_uchar) -> time_t {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    return now + 3600 / 4 * (tz as i64 - 48);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn OS__network_connected() -> bool {
    true
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn OS__load_hardware_mac(mac: *mut c_uchar) -> bool {
    // Fallback to a software mac if interface not recognised
    let fallback = Some(mac_address::MacAddress::new([0u8; 6]));

    // Returns the mac address of the first interface if multiple active
    let addr = mac_address::get_mac_address()
        .or::<Option<mac_address::MacAddress>>(Ok(fallback))
        .unwrap()
        .unwrap()
        .bytes()
        .as_mut_ptr();

    unsafe {
        std::ptr::copy(addr, mac, 6);
    };

    return true;
}

#[no_mangle]
#[allow(non_snake_case)]
#[cfg(target_os = "linux")]
///
/// Concepts borrowed from <https://github.com/systemd/systemd/blob/main/src/shutdown/shutdown.c>
pub extern "C" fn OS__reboot_dev() {
    // @TODO: Allow specifying a custom shutdown/reboot script
    /* match system_shutdown::reboot() {
        Ok(_) => info!("Rebooting"),
        Err(error) => error!("Failed to reboot: {}", error),
    } */
    if !cfg!(demo) {
        // `CAP_IPC_LOCK` is required
        //nix::sys::mman::mlockall(nix::sys::mman::MlockAllFlags::MCL_CURRENT | nix::sys::mman::MlockAllFlags::MCL_FUTURE)
        //sync_with_progress()
        //disable_coredumps()
        //disable_binfmt()
        // `CAP_KILL` is required
        //broadcast_signal(SIGTERM, true, true, arg_timeout)
        //broadcast_signal(SIGKILL, true, true, arg_timeout)

        //nix::unistd::sync();
        //thread::sleep(std::time::Duration::from_millis(4000));
        // `CAP_SYS_BOOT` required
        //nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_AUTOBOOT).expect("Reboot failed");

        system::reboot();
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn OS__update_dev() {
    // @TODO: Implement crate *self_update* for updates
}

#[no_mangle]
#[allow(non_snake_case)]
/// Initialize GPIO, controller variables, LCD, etc.
pub extern "C" fn OS__begin() {
    let gpio = Gpio::new().expect("Error getting GPIO chip");

    let mut shift_register_oe = gpio
        .get(GPIO_SHIFT_REGISTER_OE)
        .expect("Error getting line")
        .into_output_high()
        .expect("Error requesting line");

    let mut _shift_register_latch = gpio
        .get(GPIO_SHIFT_REGISTER_LATCH)
        .expect("Error getting line")
        .into_output_high()
        .expect("Error requesting line");

    let mut _shift_register_clock = gpio
        .get(GPIO_SHIFT_REGISTER_CLOCK)
        .expect("Error getting line")
        .into_output()
        .expect("Error requesting line");

    let mut _shift_register_data = gpio
        .get(GPIO_SHIFT_REGISTER_DATA)
        .expect("Error getting line")
        .into_output_high()
        .expect("Error requesting line");

    // Reset all stations (before enabling shift register)
    clear_all_station_bits();
    apply_all_station_bits();

    // pull low to enable
    shift_register_oe.set_low();

    // Sensor setup
    if cfg!(use_sensor_1) {
        let mut _sensor_1 = gpio
            .get(GPIO_SENSOR_1)
            .expect("Error getting line")
            .into_input_pullup()
            .expect("Error requesting line");
    }
    if cfg!(use_sensor_1) {
        let mut _sensor_2 = gpio
            .get(GPIO_SENSOR_2)
            .expect("Error getting line")
            .into_input_pullup()
            .expect("Error requesting line");
    }

    // RF data pin
    if cfg!(use_rf) {
        let mut _rf_transmitter = gpio
            .get(GPIO_RF_TX)
            .expect("Error getting line")
            .into_output_low()
            .expect("Error requesting line");
    }

    // Default controller status variables
    // Static variables are assigned 0 by default
    // so only need to initialize non-zero ones
    status.enabled = 1;
    status.safe_reboot = 0;

    old_status = status;

    nvdata.sunrise_time = 360; // 0600 default sunrise
    nvdata.sunset_time = 1080; // 1800 default sunrise

    nboards = 1;

    nstations = nboards * 8;

    // @TODO: Log runtime path?
}

struct IntegerOptions {
    /// firmware version
    fwv: u16,
    /// default time zone: UTC
    tz: u8,
    /// this and the next unsigned char define HTTP port
    hp0: u8,
    /// -
    hp1: u8,
    /// -
    hwv: u8,
    /// number of 8-station extension board. 0: no extension boards
    ext: u8,
    /// station delay time (-10 minutes to 10 minutes).
    sdt: u8,
    /// index of master station. 0: no master station
    mas: u8,
    /// master on time adjusted time (-10 minutes to 10 minutes)
    mton: u8,
    /// master off adjusted time (-10 minutes to 10 minutes)
    mtof: u8,
    /// water level (default 100%),
    wl: u8,
    /// device enable
    den: u8,
    /// lcd contrast
    con: u8,
    /// lcd backlight
    lit: u8,
    /// lcd dimming
    dim: u8,
    /// weather algorithm (0 means not using weather algorithm)
    uwt: u8,
    /// enable logging: 0: disable; 1: enable.
    lg: u8,
    /// index of master2. 0: no master2 station
    mas2: u8,
    /// master2 on adjusted time
    mton2: u8,
    /// master2 off adjusted time
    mtof2: u8,
    /// firmware minor version
    fwm: u8,
    /// this and next unsigned char define flow pulse rate (100x)
    fpr0: u8,
    /// default is 1.00 (100)
    fpr1: u8,
    /// set as remote extension
    re: u8,
    /// special station auto refresh
    sar: u8,
    /// ifttt enable bits
    ife: u8,
    /// sensor 1 type (see SENSOR_TYPE macro defines)
    sn1t: u8,
    /// sensor 1 option. 0: normally closed; 1: normally open.	default 1.
    sn1o: u8,
    /// sensor 2 type
    sn2t: u8,
    /// sensor 2 option. 0: normally closed; 1: normally open. default 1.
    sn2o: u8,
    /// sensor 1 on delay
    sn1on: u8,
    /// sensor 1 off delay
    sn1of: u8,
    /// sensor 2 on delay
    sn2on: u8,
    /// sensor 2 off delay
    sn2of: u8,
    /// reset
    reset: u8,
}

struct StringOptions {
    /// Device key AKA password
    dkey: [char; MAX_SOPTS_SIZE],
    /// Device location (decimal coordinates)
    /// @TODO: Represent as a vector using [f64] instead of a string. This means dropping support for using city name / postal code, but geocoder can find coordinates anyways.
    loc: [char; MAX_SOPTS_SIZE],
    /// Javascript URL for the web app
    jsp: [char; MAX_SOPTS_SIZE],
    /// Weather Service URL
    wsp: [char; MAX_SOPTS_SIZE],
    /// Weather adjustment options
    /// This data is specific to the weather adjustment method.
    wto: [char; MAX_SOPTS_SIZE],
    /// IFTTT Webhooks API key
    ifkey: [char; MAX_SOPTS_SIZE],
    /// Wi-Fi ESSID
    #[deprecated(since = "3.0.0")]
    ssid: [char; MAX_SOPTS_SIZE],
    /// Wi-Fi PSK
    #[deprecated(since = "3.0.0")]
    pass: [char; MAX_SOPTS_SIZE],
    /// MQTT config @TODO: Use a struct?
    mqtt: [char; MAX_SOPTS_SIZE],
}

#[derive(PartialEq, Debug)]
struct RocketState(&'static str);

struct OpenSprinkler {
    iopts: IntegerOptions,
    sopts: StringOptions,

    rocket: rocket::Rocket,
}

impl OpenSprinkler {
    pub fn new() -> OpenSprinkler {
        OpenSprinkler {
            // Initalize defaults
            iopts: IntegerOptions {
                fwv: 300, // @TODO: Get firmware version from cargo
                tz: 48,
                hp0: 80,
                hp1: 0,
                hwv: HARDWARE_VERSION,
                ext: 0,
                sdt: 120,
                mas: 0,
                mton: 120,
                mtof: 120,
                wl: 100,
                den: 1,
                con: 150,
                lit: 100,
                dim: 50,
                uwt: 0,
                lg: 1,
                mas2: 0,
                mton2: 120,
                mtof2: 120,
                fwm: 0, // @TODO: Get firmware version from cargo
                fpr0: 100,
                fpr1: 0,
                re: 0,
                sar: 0,
                ife: 0,
                sn1t: 0,
                sn1o: 1,
                sn2t: 0,
                sn2o: 1,
                sn1on: 0,
                sn1of: 0,
                sn2on: 0,
                sn2of: 0,
                reset: 0,
            },
            sopts: StringOptions {
                dkey: format!("{:x}", md5::compute(b"opendoor"))
                    .chars()
                    .collect::<Vec<char>>()
                    .try_into()
                    .unwrap(), // @TODO Use modern hash like Argon2
                loc: "0,0".chars().collect::<Vec<char>>().try_into().unwrap(),
                jsp: "https://ui.opensprinkler.com"
                    .chars()
                    .collect::<Vec<char>>()
                    .try_into()
                    .unwrap(),
                wsp: "weather.opensprinkler.com"
                    .chars()
                    .collect::<Vec<char>>()
                    .try_into()
                    .unwrap(),
                wto: "".chars().collect::<Vec<char>>().try_into().unwrap(),
                ifkey: "".chars().collect::<Vec<char>>().try_into().unwrap(),
                ssid: "".chars().collect::<Vec<char>>().try_into().unwrap(),
                pass: "".chars().collect::<Vec<char>>().try_into().unwrap(),
                mqtt: "".chars().collect::<Vec<char>>().try_into().unwrap(),
            },

            // Initalize Rocket
            rocket: rocket::Rocket::custom(
                rocket::Config::build(rocket::config::Environment::Staging)
                    //.address("1.2.3.4")
                    .port(80)
                    .finalize()
                    .unwrap(),
            ),
        }
    }

    pub fn start_network(&self) {
        let port: u16 = if cfg!(demo) {
            80
        } else {
            (self.iopts.hp1 as u16) << 8 + &self.iopts.hp0.into()
        };

        rocket::Rocket::ignite().launch();
    }

    pub fn set_station_name(station: usize) -> Result<(), _> {
        
    }

    /// Get station type
    pub fn get_station_type(station: usize) -> Result<StationType, &'static str> {
        let mut file = File::open(DataFile::STATIONS).expect("Stations file not found");
        let mut buf = [0u8; 1];
        file.read_exact_at(&mut buf, (248 * station as u64) + STATION_NAME_SIZE as u64 + 4);

        match u8::from_ne_bytes(buf) {
            0x00 => Ok(StationType::Standard),
            0x01 => Ok(StationType::RadioFrequency),
            0x02 => Ok(StationType::Remote),
            0x03 => Ok(StationType::GPIO),
            0x04 => Ok(StationType::HTTP),
            0xFF => Ok(StationType::Other),
            _ => Err("Unknown station type"),
        }
    }

    /// Switch Special Station
    pub fn switch_special_station(&self, station: usize, value: bool) {
        let station_type = self.get_station_type(station);
        // check if station is "special"
        if station_type == StationType::Standard {
            return ();
        }

        let data = self.get_station_data(station);
        match station_type {
            StationType::RadioFrequency => self.switch_rf_station(data.sped, value),
            StationType::Remote => self.switch_remote_station(data.sped, value),
            StationType::GPIO => self.switch_gpio_station(data.sped, value),
            StationType::HTTP => self.switch_http_station(data.sped, value),
            // Nothing to do for [StationType::Standard] and [StationType::Other]
            _ => (),
        }
    }

    /// Set station bit
    /// 
    /// This function sets the corresponding station bit. [apply_all_station_bits()] must be called after to apply the bits (which results in physically actuating the valves).
    pub fn set_station_bit(&mut self, station: usize, value: bool) {
        // Pointer to the station byte
        let mut data = *(self.station_bits + (station >> 3));
        // Mask
        let mask = 1 << (station & 0x07);

        if value {
            if data & mask {
                // If bit is already set, return "no change"
                return 0;
            } else {
                (*data) = (*data) | mask;
                // Handle special stations
                self.switch_special_station(station, true);
                return 1;
            }
        } else {
            if !((*data) & mask) {
                // If bit is already set, return "no change"
                return 0;
            } else {
                (*data) = (*data) & !(mask);
                // Handle special stations
                self.switch_special_station(station, false);
                return 255;
            }
        }

        return 0;
    }

    /// Clear all station bits
    pub fn clear_all_station_bits(&self) {
        for i in 0..MAX_NUM_STATIONS {
            self.set_station_bit(i, false);
        }
    }
}
