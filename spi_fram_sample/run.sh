#!/bin/bash
west build -b esp32c3_devkitm . && west flash && minicom --device /dev/ttyUSB0
