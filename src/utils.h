/* OpenSprinkler Unified (RPI/LINUX) Firmware
 * Copyright (C) 2015 by Ray Wang (ray@opensprinkler.com)
 *
 * Utility functions header file
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

#ifndef _UTILS_H
#define _UTILS_H

// headers for RPI
#include <stdio.h>
#include <limits.h>
#include <sys/time.h>

#include "defines.h"

// File reading/writing functions
void write_to_file(const char *fname, const char *data, unsigned long size, unsigned long pos = 0, bool trunc = true);
void read_from_file(const char *fname, char *data, unsigned long maxsize = TMP_BUFFER_SIZE, int pos = 0);
void remove_file(const char *fname);
extern "C" bool file_exists(const char *fname);

// extern "C" void file_read_block(const char *fname, void *dst, unsigned long pos, unsigned long len);
void file_read_block(const char *fname, void *dst, unsigned long pos, unsigned long len);
void file_write_block(const char *fname, const void *src, unsigned long pos, unsigned long len);
void file_copy_block(const char *fname, unsigned long from, unsigned long to, unsigned long len, void *tmp = 0);
unsigned char file_read_byte(const char *fname, unsigned long pos);
void file_write_byte(const char *fname, unsigned long pos, unsigned char v);
unsigned char file_cmp_block(const char *fname, const char *buf, unsigned long pos);

// misc. string and time converstion functions
void strncpy_P0(char *dest, const char *src, int n);
unsigned long water_time_resolve(uint16_t v);
extern "C" unsigned char water_time_encode_signed(int16_t i);
extern "C" int16_t water_time_decode_signed(unsigned char i);
void urlDecode(char *);
void peel_http_header(char *);

// Arduino compatible functions for RPI
extern "C" char *get_runtime_path();
extern "C" char *get_filename_fullpath(const char *filename);
void delay(unsigned long ms);
void delayMicroseconds(unsigned long us);
void delayMicrosecondsHard(unsigned long us);
unsigned long millis();
unsigned long micros();
void initialiseEpoch();

#endif // _UTILS_H
