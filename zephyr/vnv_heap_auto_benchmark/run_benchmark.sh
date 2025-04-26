#!/usr/bin/env bash

west build -b esp32c3_devkitm . -- -DCONFIG_MAIN_STACK_SIZE="$CONFIG_MAIN_STACK_SIZE"
status=$?

# only run this command if building was successful and FLASH != 0
if [ "$status" == "0" ] && [ "$FLASH" != "0" ]; then
    # flash and inspect output
    if [ $# -eq 1 ]; then
        west flash && python3 record_benchmark.py "$1"
    else
        west flash && python3 record_benchmark.py
    fi
fi