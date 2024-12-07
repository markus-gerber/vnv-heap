# Purpose of this file is to record the benchmark log of the esp32c3
# and save it into a file (which can be used to analyze the data later)

import serial
import datetime
import os

# Define the serial port and baud rate
serial_port = '/dev/ttyUSB0'
baud_rate = 115200

# File name for saving data
filename = "info.txt"

# Initialize the serial connection
ser = serial.Serial(serial_port, baud_rate)

try:
    with open(filename, "w") as file:
        while True:
            # Read a line from the serial port
            try:
                line_data = ser.readline()
                line = line_data.decode().strip()
            except UnicodeDecodeError:
                print("ERROR: could not parse line data " + str(line_data))
                continue

            print(line)

            if not "FINISHED" in line:
                file.write(line)
                file.write("\n")
            else:
                break

        print()
        print("Saved output file to \"" + filename + "\"")

# Handle keyboard interrupt to exit cleanly
except KeyboardInterrupt:
    while True:
        i = input("Interrupted. Do you want to keep the output file? (y/n) ")

        if i == "n":
            # recording got interrupted, delete file
            os.unlink(filename)
            break

        if i == "y":
            break

finally:
    ser.close()
