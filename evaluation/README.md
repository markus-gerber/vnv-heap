# vNV-Heap: An Ownership-based Virtually Non-Volatile Heap for Embedded Systems (Artifact)

This document contains instructions on how to set up and test the evaluation artifact for the paper "*vNV-Heap: An Ownership-based Virtually Non-Volatile Heap for Embedded Systems*".

## Getting Started Guide

Start by navigating to the `artifact` directory which contains a copy of https://gitos.rrze.fau.de/i4/openaccess/vnv-heap

``` sh
cd artifact
```

For your convenience, the development and evaluation environment can easily be set-up via docker containers.
To do this, please follow the next steps.

First, install Docker: [https://docs.docker.com/engine/install/](https://docs.docker.com/engine/install/).

Then run the main script to build and enter the development environment:

```bash
./docker-run
```

The script has been tested on Fedora 42. 

*Note*: Running this script will take some time on the first run.
This is because this script does not download a pre-built docker image, but manually builds it (e.g. by installing Rust, Zephyr, Rust support for Zephyr and a Python environment used by the evaluation plots).

Once the docker image was built, a container is started and a bash shell is opened.

*Note*: For your convenience, *(1)* the directory containing the `vnv_heap` repository, *(2)* network, and *(3)* all devices are shared between your host machine and the docker container.

Inside the docker container, you can now run the following scripts:

```bash
scripts/
├── desktop_run_benchmarks.sh     # Run all benchmarks (except for the persist benchmark) on the desktop machine. This is not meant for any latency measurements, but for debugging/testing.
├── desktop_run_testsuite.sh      # Run the whole testsuite for the vNV-Heap library
├── esp32c3_build_benchmarks.sh   # Build image for one specific or all benchmarks
├── esp32c3_run_benchmarks.sh     # Build and run one specific or all benchmarks. Note: You nee
├── notebooks_generate_plots.sh   # Generate the plots using the existing Jupyter notebooks
└── notebooks_start_server.sh     # Start the graphical Jupyter Notebook server. This can be used for example to choose select different raw data to be used for the plots
```

## Step-by-Step Instructions

*Note*: The following instructions require the docker development container introduced in the [Getting Started Guide](#getting-started-guide).

### Running Benchmarks & Measuring Latency

All of the latency measurements used for evaluations require *Espressif's ESP32-C3* microcontroller connected over *SPI* to a *Fujitsu MB85RS64V FRAM* module.
Reproducing the values from these evaluations cannot be achieved inside the virtual machine for the artifact evaluation, since the exact hardware setup is required to carry out the evaluations.

Follow the next steps to run benchmarks on the target device:

1. Connect the FRAM chip to the ESP32-C3 as follows:
    - SCK: Pin 6
    - MISO: Pin 2
    - MOSI: Pin 7
    - CS: Pin 1
2. Connect the ESP32-C3 with your machine.
3. Check the path to the connected ESP32-C3. If this differs from `/dev/ttyUSB0` update `serial_port` in `zephyr/vnv_heap_auto_benchmark/record_benchmark.py`.
4. Check the baud rate of the connected ESP32-C3. If this differs from `115200` update `baud_rate` in `zephyr/vnv_heap_auto_benchmark/record_benchmark.py`.
5. If your docker docker development container is currently running, stop it. This is required, as the development container does not support hot plugging.
6. Start the development container by running the `docker-run` script.
7. Finally, run the `esp32c3_run_benchmarks.sh` and select the benchmark you want to run. Note that running the benchmarks takes a long time (especially for the queue and the key-value store)! For all four benchmarks this might take up to one day. To reduce this time, you might reduce the amount of repetitions (`VNV_HEAP_REPETITIONS`) in `scripts/esp32c3_run_benchmarks.sh` and reduce the iteration count (`ITERATION_COUNT`) in both `vnv_heap/src/benchmarks/applications/key_value_store/runner.rs` and `vnv_heap/src/benchmarks/applications/queue/runner.rs`.

The resulting measurements are automatically saved to a *.json* file, which can be used for further analysis or for plotting.

### Plotting Measured Data

To plot measured data (saved as *.json* files) and therefore reproduce Figures 3-7, use the Jupyter Notebooks stored in `evaluation/`.

First, start the local Jupyter server with `scripts/notebooks_start_server.sh` and open the displayed URL (e.g. `http://localhost:8888/lab?token=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`) in a browser.\
You may use your browser of your host system for that.

Now, open the notebook that you are interested in.

*Optionally*: If you want to plot other measurements than the one used in the paper, update `file_name` (at the top of the notebook).

Now, run all cells to generate the new plot.\
The generated plot is also saved in `evaluation/figures/` for your convenience.

If you want to generate the plots for all notebooks, run the following script: `scripts/notebooks_generate_plots.sh`.

### Testing

The implementation of the vNV-Heap can be tested on desktop machines by:

1. Running the benchmarks (except for the persist benchmarks): `scripts/desktop_run_benchmarks.sh` (this will run for several minutes)
2. Running the testsuite: `scripts/desktop_run_testsuite.sh`
