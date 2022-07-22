/* OpenSprinkler Unified (RPI/LINUX) Firmware
 * Copyright (C) 2015 by Ray Wang (ray@opensprinkler.com)
 *
 * Utility functions
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

#include "utils.h"
#include "OpenSprinkler.h"
extern OpenSprinkler os;

/* DO NOT PORT, Use equivalent in ported func */
void delay(unsigned long howLong) {
	struct timespec sleeper, dummy;

	sleeper.tv_sec = (time_t)(howLong / 1000);
	sleeper.tv_nsec = (long)(howLong % 1000) * 1000000;

	nanosleep(&sleeper, &dummy);
}

/* DO NOT PORT, Use equivalent in ported func */
void delayMicrosecondsHard(unsigned long howLong) {
	struct timeval tNow, tLong, tEnd;

	gettimeofday(&tNow, NULL);
	tLong.tv_sec = howLong / 1000000;
	tLong.tv_usec = howLong % 1000000;
	timeradd(&tNow, &tLong, &tEnd);

	while (timercmp(&tNow, &tEnd, <))
		gettimeofday(&tNow, NULL);
}

/* DO NOT PORT, Use equivalent in ported func */
void delayMicroseconds(unsigned long howLong) {
	struct timespec sleeper;
	unsigned int uSecs = howLong % 1000000;
	unsigned int wSecs = howLong / 1000000;

	/**/ if (howLong == 0)
		return;
	else if (howLong < 100)
		delayMicrosecondsHard(howLong);
	else
	{
		sleeper.tv_sec = wSecs;
		sleeper.tv_nsec = (long)(uSecs * 1000L);
		nanosleep(&sleeper, NULL);
	}
}

static uint64_t epochMilli, epochMicro;

void initialiseEpoch()
{
	struct timeval tv;

	gettimeofday(&tv, NULL);
	epochMilli = (uint64_t)tv.tv_sec * (uint64_t)1000 + (uint64_t)(tv.tv_usec / 1000);
	epochMicro = (uint64_t)tv.tv_sec * (uint64_t)1000000 + (uint64_t)(tv.tv_usec);
}

unsigned long millis(void) {
	struct timeval tv;
	uint64_t now;

	gettimeofday(&tv, NULL);
	now = (uint64_t)tv.tv_sec * (uint64_t)1000 + (uint64_t)(tv.tv_usec / 1000);

	return (unsigned long)(now - epochMilli);
}

unsigned long micros(void) {
	struct timeval tv;
	uint64_t now;

	gettimeofday(&tv, NULL);
	now = (uint64_t)tv.tv_sec * (uint64_t)1000000 + (uint64_t)tv.tv_usec;

	return (unsigned long)(now - epochMicro);
}

/* DO NOT PORT, Use equivalent in ported func */
void write_to_file(const char *fn, const char *data, unsigned long size, unsigned long pos, bool trunc) {
	FILE *file;
	if (trunc)
	{
		file = fopen(get_filename_fullpath(fn), "wb");
	}
	else
	{
		file = fopen(get_filename_fullpath(fn), "r+b");
		if (!file)
			file = fopen(get_filename_fullpath(fn), "wb");
	}
	if (!file)
		return;
	fseek(file, pos, SEEK_SET);
	fwrite(data, 1, size, file);
	fclose(file);
}

/* DO NOT PORT, Use equivalent in ported func */
void read_from_file(const char *fn, char *data, unsigned long maxsize, unsigned long pos) {
	FILE *file;
	file = fopen(get_filename_fullpath(fn), "rb");
	if (!file)
	{
		data[0] = 0;
		return;
	}

	int res;
	fseek(file, pos, SEEK_SET);
	if (fgets(data, maxsize, file))
	{
		res = strlen(data);
	}
	else
	{
		res = 0;
	}
	if (res <= 0)
	{
		data[0] = 0;
	}

	data[maxsize - 1] = 0;
	fclose(file);
	return;
}

/* DO NOT PORT, Use equivalent in ported func */
void remove_file(const char *fn)
{
	remove(get_filename_fullpath(fn));
}

// file functions
/* DO NOT PORT, Use equivalent in ported func */
void file_read_block(const char *fn, void *dst, unsigned long pos, unsigned long len) {
	FILE *fp = fopen(get_filename_fullpath(fn), "rb");
	if (fp)
	{
		fseek(fp, pos, SEEK_SET);
		fread(dst, 1, len, fp);
		fclose(fp);
	}
}

/* DO NOT PORT, Use equivalent in ported func */
void file_write_block(const char *fn, const void *src, unsigned long pos, unsigned long len) {
	FILE *fp = fopen(get_filename_fullpath(fn), "rb+");
	if (!fp)
	{
		fp = fopen(get_filename_fullpath(fn), "wb+");
	}
	if (fp)
	{
		fseek(fp, pos, SEEK_SET); // this fails silently without the above change
		fwrite(src, 1, len, fp);
		fclose(fp);
	}
}

/* DO NOT PORT, Use equivalent in ported func */
void file_copy_block(const char *fn, unsigned long from, unsigned long to, unsigned long len, void *tmp) {
	// assume tmp buffer is provided and is larger than len
	// TODO future: if tmp buffer is not provided, do byte-to-unsigned char copy
	if (tmp == NULL)
	{
		return;
	}
	FILE *fp = fopen(get_filename_fullpath(fn), "rb+");
	if (!fp)
		return;
	fseek(fp, from, SEEK_SET);
	fread(tmp, 1, len, fp);
	fseek(fp, to, SEEK_SET);
	fwrite(tmp, 1, len, fp);
	fclose(fp);
}

// compare a block of content
/* DO NOT PORT, Use equivalent in ported func */
unsigned char file_cmp_block(const char *fn, const char *buf, unsigned long pos) {
	FILE *fp = fopen(get_filename_fullpath(fn), "rb");
	if (fp)
	{
		fseek(fp, pos, SEEK_SET);
		char c = fgetc(fp);
		while (*buf && (c == *buf))
		{
			buf++;
			c = fgetc(fp);
		}
		fclose(fp);
		return (*buf == c) ? 0 : 1;
	}

	return 1;
}

/* DO NOT PORT, Use equivalent in ported func */
unsigned char file_read_byte(const char *fn, unsigned long pos) {
	unsigned char v = 0;
	file_read_block(fn, &v, pos, 1);
	return v;
}

/* DO NOT PORT, Use equivalent in ported func */
void file_write_byte(const char *fn, unsigned long pos, unsigned char v) {
	file_write_block(fn, &v, pos, 1);
}

// copy n-character string from program memory with ending 0
/* DO NOT PORT, Use equivalent in ported func */
void strncpy_P0(char *dest, const char *src, int n)
{
	unsigned char i;
	for (i = 0; i < n; i++)
	{
		*dest = *(src++);
		dest++;
	}
	*dest = 0;
}

// resolve water time
/* special values:
 * 65534: sunrise to sunset duration
 * 65535: sunset to sunrise duration
 */
unsigned long water_time_resolve(uint16_t v) {
	if (v == 65534)
	{
		return (os.nvdata.sunset_time - os.nvdata.sunrise_time) * 60L;
	}
	else if (v == 65535)
	{
		return (os.nvdata.sunrise_time + 1440 - os.nvdata.sunset_time) * 60L;
	}
	else
	{
		return v;
	}
}

/** Convert a single hex digit character to its integer value */
/* DO NOT PORT, Use equivalent in ported func */
static unsigned char h2int(char c)
{
	if (c >= '0' && c <= '9')
	{
		return ((unsigned char)c - '0');
	}
	if (c >= 'a' && c <= 'f')
	{
		return ((unsigned char)c - 'a' + 10);
	}
	if (c >= 'A' && c <= 'F')
	{
		return ((unsigned char)c - 'A' + 10);
	}
	return (0);
}

/** Decode a url string e.g "hello%20joe" or "hello+joe" becomes "hello joe" */
/* DO NOT PORT, Use equivalent in ported func */
void urlDecode(char *urlbuf)
{
	if (!urlbuf)
		return;
	char c;
	char *dst = urlbuf;
	while ((c = *urlbuf) != 0)
	{
		if (c == '+')
			c = ' ';
		if (c == '%')
		{
			c = *++urlbuf;
			c = (h2int(c) << 4) | h2int(*++urlbuf);
		}
		*dst++ = c;
		urlbuf++;
	}
	*dst = '\0';
}

/* DO NOT PORT, Use equivalent in ported func */
void peel_http_header(char *buffer)
{ // remove the HTTP header
	uint16_t i = 0;
	bool eol = true;
	while (i < ETHER_BUFFER_SIZE)
	{
		char c = buffer[i];
		if (c == 0)
			return;
		if (c == '\n' && eol)
		{
			// copy
			i++;
			int j = 0;
			while (i < ETHER_BUFFER_SIZE)
			{
				buffer[j] = buffer[i];
				if (buffer[j] == 0)
					break;
				i++;
				j++;
			}
			return;
		}
		if (c == '\n')
		{
			eol = true;
		}
		else if (c != '\r')
		{
			eol = false;
		}
		i++;
	}
}
