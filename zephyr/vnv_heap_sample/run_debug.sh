#!/usr/bin/env bash

# enable debug assertions
export CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true

west build -b esp32c3_devkitm .
status=$?

# only run this command if building was successful and FLASH != 0
if [ "$status" == "0" ] && [ "$FLASH" != "0" ]; then
    # flash and inspect output
    west flash
    status=$?

    # remove env variable to not distract other release builds
    unset CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS


    if [ "$status" == "0" ]; then
        minicom --device /dev/ttyUSB0
    fi
else
    # remove env variable to not distract other release builds
    unset CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS
fi
