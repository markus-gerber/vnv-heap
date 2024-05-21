# Purpose of this file is to record the benchmark log of the esp32c3
# and save it into a file (which can be used to analyze the data later)

import serial
import datetime
import os

# Define the serial port and baud rate
serial_port = '/dev/ttyUSB0'
baud_rate = 115200

# File name for saving data
filename = str(datetime.datetime.now()).replace(":", "-")[:19] + ".json"
dirname = "output"

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
            line = ser.readline().decode().strip()

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
                if arg == "Finished":
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

finally:
    ser.close()
