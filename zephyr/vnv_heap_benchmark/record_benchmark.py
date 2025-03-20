# Copyright (C) 2025  Markus Elias Gerber
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

# Purpose of this file is to record the benchmark log of the esp32c3
# and save it into a file (which can be used to analyze the data later)

import serial
import datetime
import os
import sys

# Define the serial port and baud rate
serial_port = '/dev/ttyUSB0'
baud_rate = 115200

# File name for saving data
if len(sys.argv) > 1:
    filename = str(datetime.datetime.now()).replace(":", "-")[:19] + " " + sys.argv[1] + ".json"
else:
    filename = str(datetime.datetime.now()).replace(":", "-")[:19] + ".json"
    
dirname = "../../evaluation/data"

# make sure the output directory exists
os.makedirs(dirname, exist_ok=True)

# Initialize the serial connection
ser = serial.Serial(serial_port, baud_rate)

try:
    with open(dirname + "/" + filename, "w") as file:
        file.write("[")
        first_obj = True
        while True:
            # Read a line from the serial port
            try:
                line_data = ser.readline()
                line = line_data.decode().strip()
            except UnicodeDecodeError:
                print("ERROR: could not parse line data " + str(line_data))
                continue

            print(line)

            if line.startswith("[BENCH-INFO]"):
                arg = line[13:]
                if not first_obj:
                    file.write(",")
                else:
                    first_obj = False

                file.write(arg)

            elif line.startswith("[BENCH-STATUS]"):
                arg = line[15:]
                if arg.startswith("Finished"):
                    break
            

        file.write("]")

        print("Saved file to \"" + dirname + "/" + filename + "\"")

# Handle keyboard interrupt to exit cleanly
except KeyboardInterrupt:
    while True:
        i = input("Interrupted. Do you want to keep the output file? (y/n) ")

        if i == "n":
            # recording got interrupted, delete file
            os.unlink(dirname + "/" + filename)
            break

        if i == "y":
            break
    
    sys.exit(1)
finally:
    ser.close()
