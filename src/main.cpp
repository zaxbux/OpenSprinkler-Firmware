/* OpenSprinkler Unified (RPI/LINUX) Firmware
 * Copyright (C) 2015 by Ray Wang (ray@opensprinkler.com)
 *
 * Main loop
 * Feb 2015 @ OpenSprinkler.com
 *
 * This file is part of the OpenSprinkler Firmware
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

#include <limits.h>

#include "OpenSprinkler.h"
#include "program.h"
#include "weather.h"
#include "opensprinkler_server.h"
#include "mqtt.h"

// header and defs for RPI
EthernetServer *m_server = 0;
EthernetClient *m_client = 0;

void reset_all_stations();
void reset_all_stations_immediate();
void push_message(int type, uint32_t lval = 0, float fval = 0.f, const char *sval = NULL);
void manual_start_program(unsigned char, unsigned char);
void remote_http_callback(char *);

// Small variations have been added to the timing values below
// to minimize conflicting events
#define CHECK_WEATHER_TIMEOUT 21613L		 // Weather check interval (in seconds)
#define CHECK_WEATHER_SUCCESS_TIMEOUT 86400L // Weather check success interval (in seconds)
#define LCD_BACKLIGHT_TIMEOUT 15			 // LCD backlight timeout (in seconds))
#define PING_TIMEOUT 200					 // Ping test timeout (in ms)
#define UI_STATE_MACHINE_INTERVAL 50		 // how often does ui_state_machine run (in ms)
#define CLIENT_READ_TIMEOUT 5				 // client read timeout (in seconds)
// Define buffers: need them to be sufficiently large to cover string option reading
char ether_buffer[ETHER_BUFFER_SIZE * 2]; // ethernet buffer, make it twice as large to allow overflow
char tmp_buffer[TMP_BUFFER_SIZE * 2];	  // scratch buffer, make it twice as large to allow overflow

// ====== Object defines ======
OpenSprinkler os; // OpenSprinkler object
ProgramData pd;	  // ProgramdData object

/* ====== Robert Hillman (RAH)'s implementation of flow sensor ======
 * flow_begin - time when valve turns on
 * flow_start - time when flow starts being measured (i.e. 2 mins after flow_begin approx
 * flow_stop - time when valve turns off (last rising edge pulse detected before off)
 * flow_gallons - total # of gallons+1 from flow_start to flow_stop
 * flow_last_gpm - last flow rate measured (averaged over flow_gallons) from last valve stopped (used to write to log file). */
unsigned long flow_begin, flow_start, flow_stop, flow_gallons;
unsigned long flow_count = 0;
unsigned char prev_flow_state = HIGH;
float flow_last_gpm = 0;

uint32_t reboot_timer = 0;

void flow_poll()
{
	unsigned char curr_flow_state = digitalRead(PIN_SENSOR1);
	if (!(prev_flow_state == HIGH && curr_flow_state == LOW))
	{ // only record on falling edge
		prev_flow_state = curr_flow_state;
		return;
	}
	prev_flow_state = curr_flow_state;
	unsigned long curr = millis();
	flow_count++;

	/* RAH implementation of flow sensor */
	if (flow_start == 0)
	{
		flow_gallons = 0;
		flow_start = curr;
	} // if first pulse, record time
	if ((curr - flow_start) < 90000)
	{
		flow_gallons = 0;
	} // wait 90 seconds before recording flow_begin
	else
	{
		if (flow_gallons == 1)
		{
			flow_begin = curr;
		}
	}
	flow_stop = curr; // get time in ms for stop
	flow_gallons++;	  // increment gallon count for each poll
					  /* End of RAH implementation of flow sensor */
}

void do_setup()
{
	initialiseEpoch();	// initialize time reference for millis() and micros()
	os.begin();			// OpenSprinkler init
	os.options_setup(); // Setup options

	pd.init(); // ProgramData init

	if (os.start_network())
	{ // initialize network
		DEBUG_PRINTLN("network established.");
		os.status.network_fails = 0;
	}
	else
	{
		DEBUG_PRINTLN("network failed.");
		os.status.network_fails = 1;
	}

	os.mqtt.init();
	os.status.req_mqtt_restart = true;
}

void write_log(unsigned char type, unsigned long curr_time);
void schedule_all_stations(unsigned long curr_time);
void turn_on_station(unsigned char sid);
void turn_off_station(unsigned char sid, unsigned long curr_time);
void process_dynamic_events(unsigned long curr_time);
void check_weather();
bool process_special_program_command(const char *, uint32_t curr_time);
void delete_log(char *name);
void handle_web_request(char *p);

/** Main Loop */
void do_loop()
{
	// handle flow sensor using polling every 1ms (maximum freq 1/(2*1ms)=500Hz)
	static unsigned long flowpoll_timeout = 0;
	if (os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW)
	{
		unsigned long curr = millis();
		if (curr != flowpoll_timeout)
		{
			flowpoll_timeout = curr;
			flow_poll();
		}
	}

	static unsigned long last_time = 0;
	static unsigned long last_minute = 0;

	unsigned char bid, sid, s, pid, qid, bitvalue;
	ProgramStruct prog;

	os.status.mas = os.iopts[IOPT_MASTER_STATION];
	os.status.mas2 = os.iopts[IOPT_MASTER_STATION_2];
	time_t curr_time = os.now_tz();

	// ====== Process Ethernet packets ======
	EthernetClient client = m_server->available();
	if (client)
	{
		while (true)
		{
			int len = client.read((uint8_t *)ether_buffer, ETHER_BUFFER_SIZE);
			if (len <= 0)
			{
				if (!client.connected())
				{
					break;
				}
				else
				{
					continue;
				}
			}
			else
			{
				m_client = &client;
				ether_buffer[len] = 0; // put a zero at the end of the packet
				handle_web_request(ether_buffer);
				m_client = 0;
				break;
			}
		}
	}

	// Start up MQTT when we have a network connection
	if (os.status.req_mqtt_restart && os.network_connected())
	{
		DEBUG_PRINTLN("req_mqtt_restart");
		os.mqtt.begin();
		os.status.req_mqtt_restart = false;
	}
	os.mqtt.loop();

	// The main control loop runs once every second
	if (curr_time != last_time)
	{

		last_time = curr_time;

		// ====== Check raindelay status ======
		if (os.status.rain_delayed)
		{
			if (curr_time >= os.nvdata.rd_stop_time)
			{ // rain delay is over
				os.raindelay_stop();
			}
		}
		else
		{
			if (os.nvdata.rd_stop_time > curr_time)
			{ // rain delay starts now
				os.raindelay_start();
			}
		}

		// ====== Check controller status changes and write log ======
		if (os.old_status.rain_delayed != os.status.rain_delayed)
		{
			if (os.status.rain_delayed)
			{
				// rain delay started, record time
				os.raindelay_on_lasttime = curr_time;
				push_message(NOTIFY_RAINDELAY, LOGDATA_RAINDELAY, 1);
			}
			else
			{
				// rain delay stopped, write log
				write_log(LOGDATA_RAINDELAY, curr_time);
				push_message(NOTIFY_RAINDELAY, LOGDATA_RAINDELAY, 0);
			}
			os.old_status.rain_delayed = os.status.rain_delayed;
		}

		// ====== Check binary (i.e. rain or soil) sensor status ======
		os.detect_binarysensor_status(curr_time);

		if (os.old_status.sensor1_active != os.status.sensor1_active)
		{
			// send notification when sensor1 becomes active
			if (os.status.sensor1_active)
			{
				os.sensor1_active_lasttime = curr_time;
				push_message(NOTIFY_SENSOR1, LOGDATA_SENSOR1, 1);
			}
			else
			{
				write_log(LOGDATA_SENSOR1, curr_time);
				push_message(NOTIFY_SENSOR1, LOGDATA_SENSOR1, 0);
			}
		}
		os.old_status.sensor1_active = os.status.sensor1_active;

		if (os.old_status.sensor2_active != os.status.sensor2_active)
		{
			// send notification when sensor1 becomes active
			if (os.status.sensor2_active)
			{
				os.sensor2_active_lasttime = curr_time;
				push_message(NOTIFY_SENSOR2, LOGDATA_SENSOR2, 1);
			}
			else
			{
				write_log(LOGDATA_SENSOR2, curr_time);
				push_message(NOTIFY_SENSOR2, LOGDATA_SENSOR2, 0);
			}
		}
		os.old_status.sensor2_active = os.status.sensor2_active;

		// ===== Check program switch status =====
		unsigned char pswitch = os.detect_programswitch_status(curr_time);
		if (pswitch > 0)
		{
			reset_all_stations_immediate(); // immediately stop all stations
		}
		if (pswitch & 0x01)
		{
			if (pd.nprograms > 0)
				manual_start_program(1, 0);
		}
		if (pswitch & 0x02)
		{
			if (pd.nprograms > 1)
				manual_start_program(2, 0);
		}

		// ====== Schedule program data ======
		unsigned long curr_minute = curr_time / 60;
		bool match_found = false;
		RuntimeQueueStruct *q;
		// since the granularity of start time is minute
		// we only need to check once every minute
		if (curr_minute != last_minute)
		{
			last_minute = curr_minute;
			// check through all programs
			for (pid = 0; pid < pd.nprograms; pid++)
			{
				pd.read(pid, &prog); // TODO future: reduce load time
				if (prog.check_match(curr_time))
				{
					// program match found
					// check and process special program command
					if (process_special_program_command(prog.name, curr_time))
						continue;

					// process all selected stations
					for (sid = 0; sid < os.nstations; sid++)
					{
						bid = sid >> 3;
						s = sid & 0x07;
						// skip if the station is a master station (because master cannot be scheduled independently
						if ((os.status.mas == sid + 1) || (os.status.mas2 == sid + 1))
							continue;

						// if station has non-zero water time and the station is not disabled
						if (prog.durations[sid] && !(os.attrib_dis[bid] & (1 << s)))
						{
							// water time is scaled by watering percentage
							unsigned long water_time = water_time_resolve(prog.durations[sid]);
							// if the program is set to use weather scaling
							if (prog.use_weather)
							{
								unsigned char wl = os.iopts[IOPT_WATER_PERCENTAGE];
								water_time = water_time * wl / 100;
								if (wl < 20 && water_time < 10) // if water_percentage is less than 20% and water_time is less than 10 seconds
																// do not water
									water_time = 0;
							}

							if (water_time)
							{
								// check if water time is still valid
								// because it may end up being zero after scaling
								q = pd.enqueue();
								if (q)
								{
									q->st = 0;
									q->dur = water_time;
									q->sid = sid;
									q->pid = pid + 1;
									match_found = true;
								}
								else
								{
									// queue is full
								}
							} // if water_time
						}	  // if prog.durations[sid]
					}		  // for sid
					if (match_found)
					{
						push_message(NOTIFY_PROGRAM_SCHED, pid, prog.use_weather ? os.iopts[IOPT_WATER_PERCENTAGE] : 100);
					}
				} // if check_match
			}	  // for pid

			// calculate start and end time
			if (match_found)
			{
				schedule_all_stations(curr_time);

				// For debugging: print out queued elements
				/*DEBUG_PRINT("en:");
				for(q=pd.queue;q<pd.queue+pd.nqueue;q++) {
					DEBUG_PRINT("[");
					DEBUG_PRINT(q->sid);
					DEBUG_PRINT(",");
					DEBUG_PRINT(q->dur);
					DEBUG_PRINT(",");
					DEBUG_PRINT(q->st);
					DEBUG_PRINT("]");
				}
				DEBUG_PRINTLN("");*/
			}
		} // if_check_current_minute

		// ====== Run program data ======
		// Check if a program is running currently
		// If so, do station run-time keeping
		if (os.status.program_busy)
		{
			// first, go through run time queue to assign queue elements to stations
			q = pd.queue;
			qid = 0;
			for (; q < pd.queue + pd.nqueue; q++, qid++)
			{
				sid = q->sid;
				unsigned char sqi = pd.station_qid[sid];
				// skip if station is already assigned a queue element
				// and that queue element has an earlier start time
				if (sqi < 255 && pd.queue[sqi].st < q->st)
					continue;
				// otherwise assign the queue element to station
				pd.station_qid[sid] = qid;
			}
			// next, go through the stations and perform time keeping
			for (bid = 0; bid < os.nboards; bid++)
			{
				bitvalue = os.station_bits[bid];
				for (s = 0; s < 8; s++)
				{
					unsigned char sid = bid * 8 + s;

					// skip master station
					if (os.status.mas == sid + 1)
						continue;
					if (os.status.mas2 == sid + 1)
						continue;
					if (pd.station_qid[sid] == 255)
						continue;

					q = pd.queue + pd.station_qid[sid];
					// check if this station is scheduled, either running or waiting to run
					if (q->st > 0)
					{
						// if so, check if we should turn it off
						if (curr_time >= q->st + q->dur)
						{
							turn_off_station(sid, curr_time);
						}
					}
					// if current station is not running, check if we should turn it on
					if (!((bitvalue >> s) & 1))
					{
						if (curr_time >= q->st && curr_time < q->st + q->dur)
						{
							turn_on_station(sid);
						} // if curr_time > scheduled_start_time
					}	  // if current station is not running
				}		  // end_s
			}			  // end_bid

			// finally, go through the queue again and clear up elements marked for removal
			int qi;
			for (qi = pd.nqueue - 1; qi >= 0; qi--)
			{
				q = pd.queue + qi;
				if (!q->dur || curr_time >= q->st + q->dur)
				{
					pd.dequeue(qi);
				}
			}

			// process dynamic events
			process_dynamic_events(curr_time);

			// activate / deactivate valves
			os.apply_all_station_bits();

			// check through runtime queue, calculate the last stop time of sequential stations
			pd.last_seq_stop_time = 0;
			unsigned long sst;
			unsigned char re = os.iopts[IOPT_REMOTE_EXT_MODE];
			q = pd.queue;
			for (; q < pd.queue + pd.nqueue; q++)
			{
				sid = q->sid;
				bid = sid >> 3;
				s = sid & 0x07;
				// check if any sequential station has a valid stop time
				// and the stop time must be larger than curr_time
				sst = q->st + q->dur;
				if (sst > curr_time)
				{
					// only need to update last_seq_stop_time for sequential stations
					if (os.attrib_seq[bid] & (1 << s) && !re)
					{
						pd.last_seq_stop_time = (sst > pd.last_seq_stop_time) ? sst : pd.last_seq_stop_time;
					}
				}
			}

			// if the runtime queue is empty
			// reset all stations
			if (!pd.nqueue)
			{
				// turn off all stations
				os.clear_all_station_bits();
				os.apply_all_station_bits();
				// reset runtime
				pd.reset_runtime();
				// reset program busy bit
				os.status.program_busy = 0;
				// log flow sensor reading if flow sensor is used
				if (os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW)
				{
					write_log(LOGDATA_FLOWSENSE, curr_time);
					push_message(NOTIFY_FLOWSENSOR, (flow_count > os.flowcount_log_start) ? (flow_count - os.flowcount_log_start) : 0);
				}

				// in case some options have changed while executing the program
				os.status.mas = os.iopts[IOPT_MASTER_STATION];	  // update master station
				os.status.mas2 = os.iopts[IOPT_MASTER_STATION_2]; // update master2 station
			}
		} // if_some_program_is_running

		// handle master
		if (os.status.mas > 0)
		{
			int16_t mas_on_adj = water_time_decode_signed(os.iopts[IOPT_MASTER_ON_ADJ]);
			int16_t mas_off_adj = water_time_decode_signed(os.iopts[IOPT_MASTER_OFF_ADJ]);
			unsigned char masbit = 0;

			for (sid = 0; sid < os.nstations; sid++)
			{
				// skip if this is the master station
				if (os.status.mas == sid + 1)
					continue;
				bid = sid >> 3;
				s = sid & 0x07;
				// if this station is running and is set to activate master
				if ((os.station_bits[bid] & (1 << s)) && (os.attrib_mas[bid] & (1 << s)))
				{
					q = pd.queue + pd.station_qid[sid];
					// check if timing is within the acceptable range
					if (curr_time >= q->st + mas_on_adj &&
						curr_time <= q->st + q->dur + mas_off_adj)
					{
						masbit = 1;
						break;
					}
				}
			}
			os.set_station_bit(os.status.mas - 1, masbit);
		}
		// handle master2
		if (os.status.mas2 > 0)
		{
			int16_t mas_on_adj_2 = water_time_decode_signed(os.iopts[IOPT_MASTER_ON_ADJ_2]);
			int16_t mas_off_adj_2 = water_time_decode_signed(os.iopts[IOPT_MASTER_OFF_ADJ_2]);
			unsigned char masbit2 = 0;
			for (sid = 0; sid < os.nstations; sid++)
			{
				// skip if this is the master station
				if (os.status.mas2 == sid + 1)
					continue;
				bid = sid >> 3;
				s = sid & 0x07;
				// if this station is running and is set to activate master
				if ((os.station_bits[bid] & (1 << s)) && (os.attrib_mas2[bid] & (1 << s)))
				{
					q = pd.queue + pd.station_qid[sid];
					// check if timing is within the acceptable range
					if (curr_time >= q->st + mas_on_adj_2 &&
						curr_time <= q->st + q->dur + mas_off_adj_2)
					{
						masbit2 = 1;
						break;
					}
				}
			}
			os.set_station_bit(os.status.mas2 - 1, masbit2);
		}

		// process dynamic events
		process_dynamic_events(curr_time);

		// activate/deactivate valves
		os.apply_all_station_bits();

		// handle reboot request
		// check safe_reboot condition
		if (os.status.safe_reboot && (curr_time > reboot_timer))
		{
			// if no program is running at the moment
			if (!os.status.program_busy)
			{
				// and if no program is scheduled to run in the next minute
				bool willrun = false;
				for (pid = 0; pid < pd.nprograms; pid++)
				{
					pd.read(pid, &prog);
					if (prog.check_match(curr_time + 60))
					{
						willrun = true;
						break;
					}
				}
				if (!willrun)
				{
					os.reboot_dev(os.nvdata.reboot_cause);
				}
			}
		}
		else if (reboot_timer && (curr_time > reboot_timer))
		{
			os.reboot_dev(REBOOT_CAUSE_TIMER);
		}

		// real-time flow count
		static unsigned long flowcount_rt_start = 0;
		if (os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW)
		{
			if (curr_time % FLOWCOUNT_RT_WINDOW == 0)
			{
				os.flowcount_rt = (flow_count > flowcount_rt_start) ? flow_count - flowcount_rt_start : 0;
				flowcount_rt_start = flow_count;
			}
		}

		// check weather
		check_weather();

		unsigned char wuf = os.weather_update_flag;
		if (wuf) {
			if ((wuf & WEATHER_UPDATE_EIP) | (wuf & WEATHER_UPDATE_WL))
			{
				// at the moment, we only send notification if water level or external IP changed
				// the other changes, such as sunrise, sunset changes are ignored for notification
				push_message(NOTIFY_WEATHER_UPDATE, (wuf & WEATHER_UPDATE_EIP) ? os.nvdata.external_ip : 0,
							 (wuf & WEATHER_UPDATE_WL) ? os.iopts[IOPT_WATER_PERCENTAGE] : -1);
			}
			os.weather_update_flag = 0;
		}
		static unsigned char reboot_notification = 1;
		if (reboot_notification)
		{
			reboot_notification = 0;
			push_message(NOTIFY_REBOOT);
		}
	}

	delay(1); // For OSPI/LINUX, sleep 1 ms to minimize CPU usage
}

/** Check and process special program command */
bool process_special_program_command(const char *pname, uint32_t curr_time) {
	if (pname[0] == ':')
	{ // special command start with :
		if (strncmp(pname, ":>reboot_now", 12) == 0)
		{
			os.status.safe_reboot = 0;	   // reboot regardless of program status
			reboot_timer = curr_time + 65; // set a timer to reboot in 65 seconds
			// this is to avoid the same command being executed again right after reboot
			return true;
		}
		else if (strncmp(pname, ":>reboot", 8) == 0)
		{
			os.status.safe_reboot = 1;	   // by default reboot should only happen when controller is idle
			reboot_timer = curr_time + 65; // set a timer to reboot in 65 seconds
			// this is to avoid the same command being executed again right after reboot
			return true;
		}
	}
	return false;
}

/** Make weather query */
void check_weather()
{
	// do not check weather if
	// - network check has failed, or
	// - the controller is in remote extension mode
	if (os.status.network_fails > 0 || os.iopts[IOPT_REMOTE_EXT_MODE])
		return;
	if (os.status.program_busy)
		return;

	unsigned long ntz = os.now_tz();
	if (os.checkwt_success_lasttime && (ntz > os.checkwt_success_lasttime + CHECK_WEATHER_SUCCESS_TIMEOUT))
	{
		// if last successful weather call timestamp is more than allowed threshold
		// and if the selected adjustment method is not manual
		// reset watering percentage to 100
		// TODO: the firmware currently needs to be explicitly aware of which adjustment methods
		// use manual watering percentage (namely methods 0 and 2), this is not ideal
		os.checkwt_success_lasttime = 0;
		if (!(os.iopts[IOPT_USE_WEATHER] == 0 || os.iopts[IOPT_USE_WEATHER] == 2))
		{
			os.iopts[IOPT_WATER_PERCENTAGE] = 100; // reset watering percentage to 100%
			wt_rawData[0] = 0;					   // reset wt_rawData and errCode
			wt_errCode = HTTP_RQT_NOT_RECEIVED;
		}
	}
	else if (!os.checkwt_lasttime || (ntz > os.checkwt_lasttime + CHECK_WEATHER_TIMEOUT))
	{
		os.checkwt_lasttime = ntz;
		GetWeather();
	}
}

/** Turn on a station
 * This function turns on a scheduled station
 */
void turn_on_station(unsigned char sid) {
	// RAH implementation of flow sensor
	flow_start = 0;

	if (os.set_station_bit(sid, 1))
	{
		push_message(NOTIFY_STATION_ON, sid);
	}
}

/** Turn off a station
 * This function turns off a scheduled station
 * and writes log record
 */
void turn_off_station(unsigned char sid, unsigned long curr_time) {
	os.set_station_bit(sid, 0);

	unsigned char qid = pd.station_qid[sid];
	// ignore if we are turning off a station that's not running or scheduled to run
	if (qid >= pd.nqueue)
		return;

	// RAH implementation of flow sensor
	if (flow_gallons > 1)
	{
		if (flow_stop <= flow_begin)
			flow_last_gpm = 0;
		else
			flow_last_gpm = (float)60000 / (float)((flow_stop - flow_begin) / (flow_gallons - 1));
	} // RAH calculate GPM, 1 pulse per gallon
	else
	{
		flow_last_gpm = 0;
	} // RAH if not one gallon (two pulses) measured then record 0 gpm

	RuntimeQueueStruct *q = pd.queue + qid;

	// check if the current time is past the scheduled start time,
	// because we may be turning off a station that hasn't started yet
	if (curr_time > q->st)
	{
		// record lastrun log (only for non-master stations)
		if (os.status.mas != (sid + 1) && os.status.mas2 != (sid + 1))
		{
			pd.lastrun.station = sid;
			pd.lastrun.program = q->pid;
			pd.lastrun.duration = curr_time - q->st;
			pd.lastrun.endtime = curr_time;

			// log station run
			write_log(LOGDATA_STATION, curr_time);
			push_message(NOTIFY_STATION_OFF, sid, pd.lastrun.duration);
		}
	}

	// dequeue the element
	pd.dequeue(qid);
	pd.station_qid[sid] = 0xFF;
}

/** Process dynamic events
 * such as rain delay, rain sensing
 * and turn off stations accordingly
 */
void process_dynamic_events(unsigned long curr_time) {
	// check if rain is detected
	bool sn1 = false;
	bool sn2 = false;
	bool rd = os.status.rain_delayed;
	bool en = os.status.enabled;

	if ((os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_RAIN || os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_SOIL) && os.status.sensor1_active)
		sn1 = true;

	if ((os.iopts[IOPT_SENSOR2_TYPE] == SENSOR_TYPE_RAIN || os.iopts[IOPT_SENSOR2_TYPE] == SENSOR_TYPE_SOIL) && os.status.sensor2_active)
		sn2 = true;

	unsigned char sid, s, bid, qid, igs, igs2, igrd;
	for (bid = 0; bid < os.nboards; bid++)
	{
		igs = os.attrib_igs[bid];
		igs2 = os.attrib_igs2[bid];
		igrd = os.attrib_igrd[bid];

		for (s = 0; s < 8; s++)
		{
			sid = bid * 8 + s;

			// ignore master stations because they are handled separately
			if (os.status.mas == sid + 1)
				continue;
			if (os.status.mas2 == sid + 1)
				continue;
			// If this is a normal program (not a run-once or test program)
			// and either the controller is disabled, or
			// if raining and ignore rain bit is cleared
			// FIX ME
			qid = pd.station_qid[sid];
			if (qid == 255)
				continue;
			RuntimeQueueStruct *q = pd.queue + qid;

			if (q->pid >= 99)
				continue; // if this is a manually started program, proceed
			if (!en)
				turn_off_station(sid, curr_time); // if system is disabled, turn off zone
			if (rd && !(igrd & (1 << s)))
				turn_off_station(sid, curr_time); // if rain delay is on and zone does not ignore rain delay, turn it off
			if (sn1 && !(igs & (1 << s)))
				turn_off_station(sid, curr_time); // if sensor1 is on and zone does not ignore sensor1, turn it off
			if (sn2 && !(igs2 & (1 << s)))
				turn_off_station(sid, curr_time); // if sensor2 is on and zone does not ignore sensor2, turn it off
		}
	}
}

/** Scheduler
 * This function loops through the queue
 * and schedules the start time of each station
 */
void schedule_all_stations(unsigned long curr_time) {
	unsigned long con_start_time = curr_time + 1;	// concurrent start time
	unsigned long seq_start_time = con_start_time;	// sequential start time

	int16_t station_delay = water_time_decode_signed(os.iopts[IOPT_STATION_DELAY_TIME]);
	// if the sequential queue has stations running
	if (pd.last_seq_stop_time > curr_time)
	{
		seq_start_time = pd.last_seq_stop_time + station_delay;
	}

	RuntimeQueueStruct *q = pd.queue;
	unsigned char re = os.iopts[IOPT_REMOTE_EXT_MODE];
	// go through runtime queue and calculate start time of each station
	for (; q < pd.queue + pd.nqueue; q++)
	{
		if (q->st)
			continue; // if this queue element has already been scheduled, skip
		if (!q->dur)
			continue; // if the element has been marked to reset, skip
		unsigned char sid = q->sid;
		unsigned char bid = sid >> 3;
		unsigned char s = sid & 0x07;

		// if this is a sequential station and the controller is not in remote extension mode
		// use sequential scheduling. station delay time apples
		if (os.attrib_seq[bid] & (1 << s) && !re)
		{
			// sequential scheduling
			q->st = seq_start_time;
			seq_start_time += q->dur;
			seq_start_time += station_delay; // add station delay time
		}
		else
		{
			// otherwise, concurrent scheduling
			q->st = con_start_time;
			// stagger concurrent stations by 1 second
			con_start_time++;
		}
		/*DEBUG_PRINT("[");
		DEBUG_PRINT(sid);
		DEBUG_PRINT(":");
		DEBUG_PRINT(q->st);
		DEBUG_PRINT(",");
		DEBUG_PRINT(q->dur);
		DEBUG_PRINT("]");
		DEBUG_PRINTLN(pd.nqueue);*/
		if (!os.status.program_busy)
		{
			os.status.program_busy = 1; // set program busy bit
			// start flow count
			if (os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW)
			{ // if flow sensor is connected
				os.flowcount_log_start = flow_count;
				os.sensor1_active_lasttime = curr_time;
			}
		}
	}
}

/** Immediately reset all stations
 * No log records will be written
 */
void reset_all_stations_immediate()
{
	os.clear_all_station_bits();
	os.apply_all_station_bits();
	pd.reset_runtime();
}

/** Reset all stations
 * This function sets the duration of
 * every station to 0, which causes
 * all stations to turn off in the next processing cycle.
 * Stations will be logged
 */
void reset_all_stations()
{
	RuntimeQueueStruct *q = pd.queue;
	// go through runtime queue and assign water time to 0
	for (; q < pd.queue + pd.nqueue; q++)
	{
		q->dur = 0;
	}
}

/** Manually start a program
 * If pid==0, this is a test program (1 minute per station)
 * If pid==255, this is a short test program (2 second per station)
 * If pid > 0. run program pid-1
 */
void manual_start_program(unsigned char pid, unsigned char uwt) {
	bool match_found = false;
	reset_all_stations_immediate();
	ProgramStruct prog;
	unsigned long dur;
	unsigned char sid, bid, s;
	if ((pid > 0) && (pid < 255))
	{
		pd.read(pid - 1, &prog);
		push_message(NOTIFY_PROGRAM_SCHED, pid - 1, uwt ? os.iopts[IOPT_WATER_PERCENTAGE] : 100, "");
	}
	for (sid = 0; sid < os.nstations; sid++)
	{
		bid = sid >> 3;
		s = sid & 0x07;
		// skip if the station is a master station (because master cannot be scheduled independently
		if ((os.status.mas == sid + 1) || (os.status.mas2 == sid + 1))
			continue;
		dur = 60;
		if (pid == 255)
			dur = 2;
		else if (pid > 0)
			dur = water_time_resolve(prog.durations[sid]);
		if (uwt)
		{
			dur = dur * os.iopts[IOPT_WATER_PERCENTAGE] / 100;
		}
		if (dur > 0 && !(os.attrib_dis[bid] & (1 << s)))
		{
			RuntimeQueueStruct *q = pd.enqueue();
			if (q)
			{
				q->st = 0;
				q->dur = dur;
				q->sid = sid;
				q->pid = 254;
				match_found = true;
			}
		}
	}
	if (match_found)
	{
		schedule_all_stations(os.now_tz());
	}
}

// ==========================================
// ====== PUSH NOTIFICATION FUNCTIONS =======
// ==========================================
void ip2string(char *str, unsigned char ip[4]) {
	sprintf(str + strlen(str), "%d.%d.%d.%d", ip[0], ip[1], ip[2], ip[3]);
}

void push_message(int type, uint32_t lval, float fval, const char *sval)
{
	static char topic[TMP_BUFFER_SIZE];
	static char payload[TMP_BUFFER_SIZE];
	char *postval = tmp_buffer;
	uint32_t volume;

	bool ifttt_enabled = os.iopts[IOPT_IFTTT_ENABLE] & type;

	// check if this type of event is enabled for push notification
	if (!ifttt_enabled && !os.mqtt.enabled())
		return;

	if (ifttt_enabled)
	{
		strcpy(postval, "{\"value1\":\"");
	}

	if (os.mqtt.enabled())
	{
		topic[0] = 0;
		payload[0] = 0;
	}

	switch (type)
	{
	case NOTIFY_STATION_ON:

		// TODO: add IFTTT support for this event as well
		if (os.mqtt.enabled())
		{
			sprintf(topic, "opensprinkler/station/%d", lval);
			strcpy(payload, "{\"state\":1}");
		}
		break;

	case NOTIFY_STATION_OFF:

		if (os.mqtt.enabled())
		{
			sprintf(topic, "opensprinkler/station/%d", lval);
			if (os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW)
			{
				sprintf(payload, "{\"state\":0,\"duration\":%d,\"flow\":%d.%02d}", (int)fval, (int)flow_last_gpm, (int)(flow_last_gpm * 100) % 100);
			}
			else
			{
				sprintf(payload, "{\"state\":0,\"duration\":%d}", (int)fval);
			}
		}
		if (ifttt_enabled)
		{
			char name[STATION_NAME_SIZE];
			os.get_station_name(lval, name);
			sprintf(postval + strlen(postval), "Station %s closed. It ran for %d minutes %d seconds.", name, (int)fval / 60, (int)fval % 60);

			if (os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW)
			{
				sprintf(postval + strlen(postval), " Flow rate: %d.%02d", (int)flow_last_gpm, (int)(flow_last_gpm * 100) % 100);
			}
		}
		break;

	case NOTIFY_PROGRAM_SCHED:

		if (ifttt_enabled)
		{
			if (sval)
				strcat(postval, "Manually scheduled ");
			else
				strcat(postval, "Automatically scheduled ");
			strcat(postval, "Program ");
			{
				ProgramStruct prog;
				pd.read(lval, &prog);
				if (lval < pd.nprograms)
					strcat(postval, prog.name);
			}
			sprintf(postval + strlen(postval), " with %d%% water level.", (int)fval);
		}
		break;

	case NOTIFY_SENSOR1:

		if (os.mqtt.enabled())
		{
			strcpy(topic, "opensprinkler/sensor1");
			sprintf(payload, "{\"state\":%d}", (int)fval);
		}
		if (ifttt_enabled)
		{
			strcat(postval, "Sensor 1 ");
			strcat(postval, ((int)fval) ? "activated." : "de-activated.");
		}
		break;

	case NOTIFY_SENSOR2:

		if (os.mqtt.enabled())
		{
			strcpy(topic, "opensprinkler/sensor2");
			sprintf(payload, "{\"state\":%d}", (int)fval);
		}
		if (ifttt_enabled)
		{
			strcat(postval, "Sensor 2 ");
			strcat(postval, ((int)fval) ? "activated." : "de-activated.");
		}
		break;

	case NOTIFY_RAINDELAY:

		if (os.mqtt.enabled())
		{
			strcpy(topic, "opensprinkler/raindelay");
			sprintf(payload, "{\"state\":%d}", (int)fval);
		}
		if (ifttt_enabled)
		{
			strcat(postval, "Rain delay ");
			strcat(postval, ((int)fval) ? "activated." : "de-activated.");
		}
		break;

	case NOTIFY_FLOWSENSOR:

		volume = os.iopts[IOPT_PULSE_RATE_1];
		volume = (volume << 8) + os.iopts[IOPT_PULSE_RATE_0];
		volume = lval * volume;
		if (os.mqtt.enabled())
		{
			strcpy(topic, "opensprinkler/sensor/flow");
			sprintf(payload, "{\"count\":%lu,\"volume\":%d.%02d}", lval, (int)volume / 100, (int)volume % 100);
		}
		if (ifttt_enabled)
		{
			sprintf(postval + strlen(postval), "Flow count: %lu, volume: %d.%02d", lval, (int)volume / 100, (int)volume % 100);
		}
		break;

	case NOTIFY_WEATHER_UPDATE:

		if (ifttt_enabled)
		{
			if (lval > 0)
			{
				strcat(postval, "External IP updated: ");
				unsigned char ip[4] = {(unsigned char)((lval >> 24) & 0xFF),
									   (unsigned char)((lval >> 16) & 0xFF),
									   (unsigned char)((lval >> 8) & 0xFF),
									   (unsigned char)(lval & 0xFF)};
				ip2string(postval, ip);
			}
			if (fval >= 0)
			{
				sprintf(postval + strlen(postval), "Water level updated: %d%%.", (int)fval);
			}
		}
		break;

	case NOTIFY_REBOOT:

		if (os.mqtt.enabled())
		{
			strcpy(topic, "opensprinkler/system");
			strcpy(payload, "{\"state\":\"started\"}");
		}
		if (ifttt_enabled)
		{
			strcat(postval, "Process restarted.");
		}
		break;
	}

	if (os.mqtt.enabled() && strlen(topic) && strlen(payload))
		os.mqtt.publish(topic, payload);

	if (ifttt_enabled)
	{
		strcat(postval, "\"}");

		// char postBuffer[1500];
		BufferFiller bf = ether_buffer;
		bf.emit_p(
			"POST /trigger/sprinkler/with/key/$O HTTP/1.0\r\n"
			"Host: $S\r\n"
			"Accept: */*\r\n"
			"Content-Length: $D\r\n"
			"Content-Type: application/json\r\n\r\n$S",
			SOPT_IFTTT_KEY, DEFAULT_IFTTT_URL, strlen(postval), postval);

		os.send_http_request(DEFAULT_IFTTT_URL, 80, ether_buffer, remote_http_callback);
	}
}

// ================================
// ====== LOGGING FUNCTIONS =======
// ================================
char LOG_PREFIX[] = "./logs/";

/** Generate log file name
 * Log files will be named /logs/xxxxx.txt
 */
void make_logfile_name(char *name)
{
	strcpy(tmp_buffer + TMP_BUFFER_SIZE - 10, name);
	strcpy(tmp_buffer, LOG_PREFIX);
	strcat(tmp_buffer, tmp_buffer + TMP_BUFFER_SIZE - 10);
	strcat(tmp_buffer, ".txt");
}

/* To save RAM space, we store log type names
 * in program memory, and each name
 * must be strictly two characters with an ending 0
 * so each name is 3 characters total
 */
static const char log_type_names[] =
	"  \0"
	"s1\0"
	"rd\0"
	"wl\0"
	"fl\0"
	"s2\0"
	"cu\0";

/** write run record to log on SD card */
void write_log(unsigned char type, unsigned long curr_time) {
	if (!os.iopts[IOPT_ENABLE_LOGGING])
		return;

	// file name will be logs/xxxxx.tx where xxxxx is the day in epoch time
	// ultoa(curr_time / 86400, tmp_buffer, 10);
	sprintf(tmp_buffer, "%lu", curr_time / 86400);
	make_logfile_name(tmp_buffer);

	// Step 1: open file if exists, or create new otherwise,
	// and move file pointer to the end
	// prepare log folder for RPI
	struct stat st;
	if (stat(get_filename_fullpath(LOG_PREFIX), &st))
	{
		if (mkdir(get_filename_fullpath(LOG_PREFIX), S_IRUSR | S_IWUSR | S_IXUSR | S_IRGRP | S_IWGRP | S_IXGRP | S_IROTH | S_IWOTH | S_IXOTH))
		{
			return;
		}
	}
	FILE *file;
	file = fopen(get_filename_fullpath(tmp_buffer), "rb+");
	if (!file)
	{
		file = fopen(get_filename_fullpath(tmp_buffer), "wb");
		if (!file)
			return;
	}
	fseek(file, 0, SEEK_END);

	// Step 2: prepare data buffer
	strcpy(tmp_buffer, "[");

	if (type == LOGDATA_STATION)
	{
		// itoa(pd.lastrun.program, tmp_buffer + strlen(tmp_buffer), 10);
		sprintf(tmp_buffer + strlen(tmp_buffer), "%d", pd.lastrun.program);
		strcat(tmp_buffer, ",");
		// itoa(pd.lastrun.station, tmp_buffer + strlen(tmp_buffer), 10);
		sprintf(tmp_buffer + strlen(tmp_buffer), "%d", pd.lastrun.station);
		strcat(tmp_buffer, ",");
		// duration is unsigned integer
		// ultoa((unsigned long)pd.lastrun.duration, tmp_buffer + strlen(tmp_buffer), 10);
		sprintf(tmp_buffer + strlen(tmp_buffer), "%lu", (unsigned long)pd.lastrun.duration);
	}
	else
	{
		unsigned long lvalue = 0;
		if (type == LOGDATA_FLOWSENSE)
		{
			lvalue = (flow_count > os.flowcount_log_start) ? (flow_count - os.flowcount_log_start) : 0;
		}
		// ultoa(lvalue, tmp_buffer + strlen(tmp_buffer), 10);
		sprintf(tmp_buffer + strlen(tmp_buffer), "%lu", lvalue);
		strcat(tmp_buffer, ",\"");
		strcat(tmp_buffer, log_type_names + type * 3);
		strcat(tmp_buffer, "\",");

		switch (type)
		{
		case LOGDATA_FLOWSENSE:
			lvalue = (curr_time > os.sensor1_active_lasttime) ? (curr_time - os.sensor1_active_lasttime) : 0;
			break;
		case LOGDATA_SENSOR1:
			lvalue = (curr_time > os.sensor1_active_lasttime) ? (curr_time - os.sensor1_active_lasttime) : 0;
			break;
		case LOGDATA_SENSOR2:
			lvalue = (curr_time > os.sensor2_active_lasttime) ? (curr_time - os.sensor2_active_lasttime) : 0;
			break;
		case LOGDATA_RAINDELAY:
			lvalue = (curr_time > os.raindelay_on_lasttime) ? (curr_time - os.raindelay_on_lasttime) : 0;
			break;
		case LOGDATA_WATERLEVEL:
			lvalue = os.iopts[IOPT_WATER_PERCENTAGE];
			break;
		}
		// ultoa(lvalue, tmp_buffer + strlen(tmp_buffer), 10);
		sprintf(tmp_buffer + strlen(tmp_buffer), "%lu", lvalue);
	}
	strcat(tmp_buffer, ",");
	// ultoa(curr_time, tmp_buffer + strlen(tmp_buffer), 10);
	sprintf(tmp_buffer + strlen(tmp_buffer), "%lu", curr_time);
	if ((os.iopts[IOPT_SENSOR1_TYPE] == SENSOR_TYPE_FLOW) && (type == LOGDATA_STATION))
	{
		// RAH implementation of flow sensor
		strcat(tmp_buffer, ",");
		sprintf(tmp_buffer + strlen(tmp_buffer), "%5.2f", flow_last_gpm);
	}
	strcat(tmp_buffer, "]\r\n");

	fwrite(tmp_buffer, 1, strlen(tmp_buffer), file);
	fclose(file);
}

/** Delete log file
 * If name is 'all', delete all logs
 */
void delete_log(char *name)
{
	if (!os.iopts[IOPT_ENABLE_LOGGING])
		return;
	// delete_log implementation for RPI
	if (strncmp(name, "all", 3) == 0)
	{
		// delete the log folder
		rmdir(get_filename_fullpath(LOG_PREFIX));
		return;
	}
	else
	{
		make_logfile_name(name);
		remove(get_filename_fullpath(tmp_buffer));
	}
}

// main function for RPI
int main(int argc, char *argv[])
{
	do_setup();

	while (true)
	{
		do_loop();
	}
	return 0;
}
