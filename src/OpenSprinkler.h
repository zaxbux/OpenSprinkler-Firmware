/* OpenSprinkler Unified (RPI/LINUX) Firmware
 * Copyright (C) 2015 by Ray Wang (ray@opensprinkler.com)
 *
 * OpenSprinkler library header file
 * Feb 2015 @ OpenSprinkler.com
 *
 * This file is part of the OpenSprinkler library
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see
 * <http://www.gnu.org/licenses/>.
 */

#ifndef _OPENSPRINKLER_H
#define _OPENSPRINKLER_H

#include "defines.h"
#include "utils.h"
#include "gpio.h"
#include "mqtt.h"
#include <time.h>
#include <string.h>
#include <unistd.h>
#include <netdb.h>
#include <sys/stat.h>
#include "etherport.h"

/** Non-volatile data structure */
struct NVConData
{
	uint16_t sunrise_time; // sunrise time (in minutes)
	uint16_t sunset_time;  // sunset time (in minutes)
	uint32_t rd_stop_time; // rain delay stop time
	uint32_t external_ip;  // external ip
	uint8_t reboot_cause;  // reboot cause
};

struct StationAttrib
{ // station attributes
	byte mas : 1;
	byte igs : 1; // ignore sensor 1
	byte mas2 : 1;
	byte dis : 1;
	byte seq : 1;
	byte igs2 : 1; // ignore sensor 2
	byte igrd : 1; // ignore rain delay
	byte unused : 1;

	byte gid : 4; // group id: reserved for the future
	byte dummy : 4;
	byte reserved[2]; // reserved bytes for the future
};					  // total is 4 bytes so far

/** Station data structure */
struct StationData
{
	char name[STATION_NAME_SIZE];
	StationAttrib attrib;
	byte type;							  // station type
	byte sped[STATION_SPECIAL_DATA_SIZE]; // special station data
};

/** RF station data structures - Must fit in STATION_SPECIAL_DATA_SIZE */
struct RFStationData
{
	byte on[6];
	byte off[6];
	byte timing[4];
};

/** Remote station data structures - Must fit in STATION_SPECIAL_DATA_SIZE */
struct RemoteStationData
{
	byte ip[8];
	byte port[4];
	byte sid[2];
};

/** GPIO station data structures - Must fit in STATION_SPECIAL_DATA_SIZE */
struct GPIOStationData
{
	byte pin[2];
	byte active;
};

/** HTTP station data structures - Must fit in STATION_SPECIAL_DATA_SIZE */
struct HTTPStationData
{
	byte data[STATION_SPECIAL_DATA_SIZE];
};

/** Volatile controller status bits */
struct ConStatus
{
	byte enabled : 1;		   // operation enable (when set, controller operation is enabled)
	byte rain_delayed : 1;	   // rain delay bit (when set, rain delay is applied)
	byte sensor1 : 1;		   // sensor1 status bit (when set, sensor1 on is detected)
	byte program_busy : 1;	   // HIGH means a program is being executed currently
	byte has_curr_sense : 1;   // HIGH means the controller has a current sensing pin
	byte safe_reboot : 1;	   // HIGH means a safe reboot has been marked
	byte req_ntpsync : 1;	   // request ntpsync
	byte req_network : 1;	   // request check network
	byte display_board : 5;	   // the board that is being displayed onto the lcd
	byte network_fails : 3;	   // number of network fails
	byte mas : 8;			   // master station index
	byte mas2 : 8;			   // master2 station index
	byte sensor2 : 1;		   // sensor2 status bit (when set, sensor2 on is detected)
	byte sensor1_active : 1;   // sensor1 active bit (when set, sensor1 is activated)
	byte sensor2_active : 1;   // sensor2 active bit (when set, sensor2 is activated)
	byte req_mqtt_restart : 1; // request mqtt restart
};

extern const char iopt_json_names[];

class OpenSprinkler
{
public:
	// data members
	static OSMqtt mqtt;

	static NVConData nvdata;
	static ConStatus status;
	static ConStatus old_status;
	static byte nboards, nstations;
	static byte hw_type; // hardware type
	static byte hw_rev;	 // hardware minor

	static byte iopts[];		// integer options
	static const char *sopts[]; // string options
	static byte station_bits[]; // station activation bits. each byte corresponds to a board (8 stations)
								// first byte-> master controller, second byte-> ext. board 1, and so on
	// TODO future: the following attribute bytes are for backward compatibility
	static byte attrib_mas[];
	static byte attrib_igs[];
	static byte attrib_mas2[];
	static byte attrib_igs2[];
	static byte attrib_igrd[];
	static byte attrib_dis[];
	static byte attrib_seq[];
	static byte attrib_spe[];

	// variables for time keeping
	static ulong sensor1_on_timer;		  // time when sensor1 is detected on last time
	static ulong sensor1_off_timer;		  // time when sensor1 is detected off last time
	static ulong sensor1_active_lasttime; // most recent time sensor1 is activated
	static ulong sensor2_on_timer;		  // time when sensor2 is detected on last time
	static ulong sensor2_off_timer;		  // time when sensor2 is detected off last time
	static ulong sensor2_active_lasttime; // most recent time sensor1 is activated
	static ulong raindelay_on_lasttime;	  // time when the most recent rain delay started
	static ulong flowcount_rt;			  // flow count (for computing real-time flow rate)
	static ulong flowcount_log_start;	  // starting flow count (for logging)

	static ulong checkwt_lasttime;		   // time when weather was checked
	static ulong checkwt_success_lasttime; // time when weather check was successful
	static ulong powerup_lasttime;		   // time when controller is powered up most recently
	static uint8_t last_reboot_cause;	   // last reboot cause
	static byte weather_update_flag;
	// member functions
	// -- setup
	static void update_dev();										 // update software for Linux instances
	static void reboot_dev(uint8_t);								 // reboot the microcontroller
	static void begin();											 // initialization, must call this function before calling other functions
	static byte start_network();									 // initialize network with the given mac and port
	static byte start_ether();										 // initialize ethernet with the given mac and port
	static bool network_connected();								 // check if the network is up
	static bool load_hardware_mac(byte *buffer);					 // read hardware mac address
	static time_t now_tz();
	// -- station names and attributes
	static void get_station_data(byte sid, StationData *data); // get station data
	static void set_station_data(byte sid, StationData *data); // set station data
	static void get_station_name(byte sid, char buf[]);		   // get station name
	static void set_station_name(byte sid, char buf[]);		   // set station name
	static byte get_station_type(byte sid);					   // get station type
	// static StationAttrib get_station_attrib(byte sid); // get station attribute
	static void attribs_save();														  // repackage attrib bits and save (backward compatibility)
	static void attribs_load();														  // load and repackage attrib bits (backward compatibility)
	static uint16_t parse_rfstation_code(RFStationData *data, ulong *on, ulong *off); // parse rf code into on/off/time sections
	static void switch_rfstation(RFStationData *data, bool turnon);					  // switch rf station
	static void switch_remotestation(RemoteStationData *data, bool turnon);			  // switch remote station
	static void switch_gpiostation(GPIOStationData *data, bool turnon);				  // switch gpio station
	static void switch_httpstation(HTTPStationData *data, bool turnon);				  // switch http station

	// -- options and data storeage
	static void nvdata_load();
	static void nvdata_save();

	static void options_setup();
	static void pre_factory_reset();
	static void factory_reset();
	static void iopts_load();
	static void iopts_save();
	static bool sopt_save(byte oid, const char *buf);
	static void sopt_load(byte oid, char *buf);
	static String sopt_load(byte oid);

	static byte password_verify(char *pw); // verify password

	// -- controller operation
	static void enable();							// enable controller operation
	static void disable();							// disable controller operation, all stations will be closed immediately
	static void raindelay_start();					// start raindelay
	static void raindelay_stop();					// stop rain delay
	static void detect_binarysensor_status(ulong);	// update binary (rain, soil) sensor status
	static byte detect_programswitch_status(ulong); // get program switch status
	static void sensor_resetall();

	static uint16_t read_current();	  // read current sensing value
	static uint16_t baseline_current; // resting state current

	static int detect_exp();	 // detect the number of expansion boards
	static byte weekday_today(); // returns index of today's weekday (Monday is 0)

	static byte set_station_bit(byte sid, byte value);		  // set station bit of one station (sid->station index, value->0/1)
	static void switch_special_station(byte sid, byte value); // swtich special station
	static void clear_all_station_bits();					  // clear all station bits
	static void apply_all_station_bits();					  // apply all station bits (activate/deactive values)

	static int8_t send_http_request(uint32_t ip4, uint16_t port, char *p, void (*callback)(char *) = NULL, uint16_t timeout = 3000);
	static int8_t send_http_request(const char *server, uint16_t port, char *p, void (*callback)(char *) = NULL, uint16_t timeout = 3000);
	static int8_t send_http_request(char *server_with_port, char *p, void (*callback)(char *) = NULL, uint16_t timeout = 3000);
	static byte engage_booster;
};

// TODO
extern EthernetServer *m_server;

#endif // _OPENSPRINKLER_H
