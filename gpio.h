/* OpenSprinkler Unified (RPI/LINUX) Firmware
 * Copyright (C) 2015 by Ray Wang (ray@opensprinkler.com)
 *
 * GPIO header file
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

#ifndef GPIO_H
#define GPIO_H

#include <sys/stat.h>
#include <fcntl.h>
#include <stdio.h>

#include "defines.h"
#define OUTPUT 0
#define INPUT 1

#if defined(OSPI)
#define INPUT_PULLUP 2
#else
#define INPUT_PULLUP INPUT
#endif

#define HIGH 1
#define LOW 0

void pinMode(int pin, byte mode);
void digitalWrite(int pin, byte value);
int gpio_fd_open(int pin, int mode = O_WRONLY);
void gpio_fd_close(int fd);
void gpio_write(int fd, byte value);
byte digitalRead(int pin);
// mode can be any of 'rising', 'falling', 'both'
void attachInterrupt(int pin, const char *mode, void (*isr)(void));

#endif // GPIO_H
