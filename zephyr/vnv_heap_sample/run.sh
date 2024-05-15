#!/bin/bash

# enable debug assertions
export CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true

west build -b esp32c3_devkitm .

# remove env variable to not distract other release builds
unset CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS

# flash and inspect output
west flash && minicom --device /dev/ttyUSB0
