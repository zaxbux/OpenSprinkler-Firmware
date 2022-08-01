============================================
==== OpenSprinkler RPI Firmware ====
============================================

This is a unified OpenSprinkler firmware for and Linux-based OpenSprinklers such as OpenSprinkler Pi.

For OSPi or other Linux-based OpenSprinkler:
https://openthings.freshdesk.com/support/solutions/articles/5000631599-installing-and-updating-the-unified-firmware

============================================
Questions and comments:
http://www.opensprinkler.com
============================================

## Changes

While porting the codebase to Rust, some changes were made to align with language principals. Other changes were simply quality of life improvements.

* Time is in UTC (timezone is strictly for display).
  * Also converted from unsigned 32-bit integers to signed 64-bit integers for time representations.
* NVConData, Options, Stations, and Programs are stored in one file as BSON (Binary JSON).
* Options that used `0` as a none/null value now use the native `None` type.
* TLS
* IPv6
