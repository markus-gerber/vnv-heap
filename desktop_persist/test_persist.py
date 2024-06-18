import subprocess
import signal
import time
import os
import sys

execution_duration = -1
if len(sys.argv) >= 2:
    execution_duration = int(sys.argv[1])

def exit_program():
    if process:
        os.kill(process.pid, signal.SIGTERM)

    sys.exit(0)

# handle ctrl c
signal.signal(signal.SIGINT, lambda _, __: exit_program())

print("> cargo build")
assert subprocess.run(["cargo", "build"]).returncode == 0

print("")
print("> ../target/debug/desktop_persist")
process = subprocess.Popen("../target/debug/desktop_persist", env={"RUST_BACKTRACE": "1"})

time.sleep(1)

start_time = time.time()
signal_delay = 0.02

while True:

    # exit program if test duration was specified and duration was reached
    if execution_duration != -1 and time.time() - start_time > execution_duration:
        exit_program()

    # check if program has already exited
    if process.poll():
        exit(process.returncode)

    # trigger vnv_persist_all
    os.kill(process.pid, signal.SIGUSR1)
    time.sleep(signal_delay)
