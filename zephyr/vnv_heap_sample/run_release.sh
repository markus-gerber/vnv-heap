#!/usr/bin/env bash

west build -b esp32c3_devkitm .
status=$?

# only run this command if building was successful and FLASH != 0
if [ "$status" == "0" ] && [ "$FLASH" != "0" ]; then
    # flash and inspect output
    west flash && minicom --device /dev/ttyUSB0
fi