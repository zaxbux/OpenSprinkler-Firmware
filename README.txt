============================================
==== OpenSprinkler AVR/RPI/BBB Firmware ====
============================================

********************************************
FIRMWARE 2.1.9(4) MODIFIED TO SUPPORT W5500
ETHERNET MODULE FOR OS 3.2. REQUIRES ARDUINO
ETHERNET LIBRARY:
https://github.com/arduino-libraries/Ethernet
AT THE MOMENT OF THIS WRITING, YOU NEED TO
MODIFY THE LIBRARY TO FIX THE ISSUE BELOW:
https://github.com/arduino-libraries/Ethernet/issues/139
SPECIFICALLY, GO TO W5100.h, UNDER SECTION
ARDUINO_ARCH_ESP8266, CHANGE setSS and resetSS
TO USE ARDUINO'S digitalWrite FUNCTION.
********************************************

This is a unified OpenSprinkler firmware for Arduino, and Linux-based OpenSprinklers such as OpenSprinkler Pi.

For OS (Arduino-based OpenSprinkler) 2.x:
https://openthings.freshdesk.com/support/solutions/articles/5000165132-how-to-compile-opensprinkler-firmware

For OSPi/OSBO or other Linux-based OpenSprinkler:
https://openthings.freshdesk.com/support/solutions/articles/5000631599-installing-and-updating-the-unified-firmware

============================================
Questions and comments:
http://www.opensprinkler.com
============================================
