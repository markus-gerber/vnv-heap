# vNV-Heap: An Ownership-based Virtually Non-Volatile Heap for Embedded Systems (Artifact)

This document contains instructions on how to set up and test the evaluation artifact for the paper "*vNV-Heap: An Ownership-based Virtually Non-Volatile Heap for Embedded Systems*".

We seek to receive the following badges:
- Artifacts Available v1.1
  - The `artifact` subdirectory in the archive is a copy of
    https://gitos.rrze.fau.de/i4/openaccess/vnv-heap which is publicly
    available (and FAU's hoster guarantees long-term availability). We
    will upload the evaluated version to Zotero with a DOI for the
    Camera-Ready version.
- Artifacts Evaluated – Functional v1.1
  - Documented: See `artifact/evaluation/README.md` (this file),
    `artifact/README.md` as well as the source code.
  - Consistent: We include the full source code and evaluation scripts for the current version of the paper.
  - Complete: We include everything except for the hardware which we can
    not include directly. If you seek to reproduce the results using the
    hardware, please contact us. We will do our best to give you
    access to the board.
  - Exercisable: See "Run the Test Suite in a VM" and "Plot Evaluation Results"
- Artifacts Evaluated – Reusable v1.1
  - We are sure you agree that our source code is very carefully
    documented and well-structured to the extent that reuse and
    repurposing is facilitated. In particular, norms and standards of
    the research community for artifacts of this type are strictly
    adhered to (e.g., we use a standard toolchain and publish our Git
    repository).
- Results Reproduced v1.1
  - See "Run Evaluation in a VM", "Run Evaluation on Hardware
    (Informative)", and "Plot Evaluation Results".

The following list of claims is supported by the artifact:

- *7.2 Reference Retrieval [Figure 3]*
  - **Claim**: Using the vNV-Heap is more energy efficient than existing approaches (due to lower runtime).
  - **Claim**: The vNV-Heap provides runtime guarantees by static worst-case bounds.
  - **Supported by**:
    - The *get_ref* benchmark to be run on the target platform (runnable through [scripts/esp32c3_run_benchmarks.sh](scripts/esp32c3_run_benchmarks.sh)). Contain the measurements of different code paths for (best- and worst-case latency)
    - The measured raw data [evaluation/data/2025-03-13 08-27-11 get.json](<evaluation/data/2025-03-13 08-27-11 get.json>)
    - The corresponding plot in [evaluation/get_ref.ipynb](evaluation/get_ref.ipynb)
- *7.3 Read/Write Cache [Figure 4]*
  - **Claim**: Using the vNV-Heap is more energy efficient than existing approaches (due to lower runtime).
  - **Claim**: The vNV-Heap only marginally reduces performance when for larger data, while increasing performance significantly for a smaller dataset compared with an exclusive Non-Volatile Memory approach.
  - **Claim**: The vNV-Heap increases the amount of usable memory compared with an exclusive volatile RAM approach.
  - **Supported by**:
    - The *queue* benchmark to be run on the target platform (runnable through [scripts/esp32c3_run_benchmarks.sh](scripts/esp32c3_run_benchmarks.sh))
    - The measured raw data [evaluation/data/2025-03-13 20-31-26 queue.json](<evaluation/data/2025-03-13 20-31-26 queue.json>)
    - The corresponding plot in [evaluation/queue.ipynb](evaluation/queue.ipynb)
- *7.4 Predictable Checkpointing [Figure 5]*
  - **Claim**: The vNV-Heap provides runtime guarantees by static worst-case bounds (regarding creating a checkpoint).
  - **Claim**: The vNV-Heap is able to limit the WCEC for persisting data by specifying the amount of state.
  - **Claim**: The vNV-Heap is able to reduce the WCEC for persisting data compared to unmanaged RAM by reducing the amount of modified state.
  - **Supported by**:
    - The *persist* benchmark to be run on the target platform (runnable through [scripts/esp32c3_run_benchmarks.sh](scripts/esp32c3_run_benchmarks.sh)) (consists of following subbenchmarks: *dirty_size*, *buffer_size*, *locked_wcet*, and *persistent_storage* - for more information, have a look at [scripts/esp32c3_run_benchmarks.sh](scripts/esp32c3_run_benchmarks.sh) and [vnv_heap/src/benchmarks/mod.rs](vnv_heap/src/benchmarks/mod.rs))
    - The measured raw data [evaluation/data/2025-03-13 16-17-54 persist.json](<evaluation/data/2025-03-13 16-17-54 persist.json>) (and [evaluation/data/2025-03-13 08-27-11 get.json](<evaluation/data/2025-03-13 08-27-11 get.json>) for *persistent_storage* subbenchmark)
    - The corresponding plot in [evaluation/persist.ipynb](evaluation/persist.ipynb)
- *7.5 Key-Value Store [Figure 6]*
  - **Claim**: Even though the vNV-Heap is more feature-rich, its performance is comparable to ManagedState for small page sizes.
  - **Claim**: For large page sizes, vNV-Heap clearly outperforms ManagedState.
  - **Supported by**:
    - The *kvs* benchmark to be run on the target platform (runnable through [scripts/esp32c3_run_benchmarks.sh](scripts/esp32c3_run_benchmarks.sh))
    - The measured raw data [evaluation/data/2025-03-19 00-00-16 kvs.json](<evaluation/data/2025-03-19 00-00-16 kvs.json>)
    - The corresponding plots in [evaluation/kvs.ipynb](evaluation/kvs.ipynb)
- *7.5 Key-Value Store (1) [Figure 7]*
  - **Claim**: The vNV-Heap can reduce the size of its internal metadata for each resident object to 3 bytes.
  - **Supported by**:
    - The comments inside the ResidentObjectManager in [vnv_heap/src/resident_object_manager/resident_object_metadata.rs](vnv_heap/src/resident_object_manager/resident_object_metadata.rs)
- *7.5 Key-Value Store (2) [Figure 7]*
  - **Claim**: Managed-State’s metadata overhead is significant for small page sizes.
  - **Claim**: The vNV-Heap has a lower metadata per chunk compared with ManagedState using page sizes 32 and 64.
  - **Supported by**:
    - The calculations in [evaluation/kvs.ipynb](evaluation/kvs.ipynb) and the corresponding plot

The following list of claims is not supported by the artifact:

- **Claim**: Managed-State’s metadata overhead is 1 byte per page.
  - This claim is not supported by this work, as it originates from the paper "Efficient State Retention through Paged Memory Management for Reactive Transient Computing"

## Getting Started Guide

Start by navigating to the `artifact` directory which contains a copy of https://gitos.rrze.fau.de/i4/openaccess/vnv-heap

``` sh
cd artifact
```

For your convenience, the development and evaluation environment can easily be set-up via Docker containers.
To do this, please follow the next steps.

First, install Docker: [https://docs.docker.com/engine/install/](https://docs.docker.com/engine/install/).

Then run the main script to build and enter the development environment:

```bash
./docker-run # 30min, 20GB free disk space
```

The script has been tested on Debian 12 Bookworm, Fedora 42 and Ubuntu
25.04. If you encounter any problems consider starting the script from
any of these distros or consider building the container manually using
`docker build` (see `Dockerfile`). You can of course also follow the
instructions from the Dockerfile to set up all dependencies on your
local machine.

*Note*: Running this script will take some time on the first run. This
is because this script does not download a pre-built Docker image, but
manually builds it (e.g. by installing Rust, Zephyr, Rust support for
Zephyr and a Python environment used by the evaluation plots). This is
required to prevent conflicts regarding the UIDs in the mounted
directories. Furthermore, the time that would be required for
downloading a prebuilt Docker image from our servers would be equivalent
to the time required to build the container (most of the time is spent
downloading dependencies).

Once the Docker image was built, the container is automatically started
and a bash shell is opened.

*Note*: For your convenience, *(1)* the directory containing the `vnv_heap` repository, *(2)* network, and *(3)* all devices are shared between your host machine and the Docker container.

Inside the Docker container, you can now run the following scripts:

```bash
scripts/
├── desktop_run_benchmarks.sh     # Run all benchmarks (except for the persist benchmark) on the desktop machine. This is not meant for any latency measurements, but for debugging/testing.
├── desktop_run_testsuite.sh      # Run the whole testsuite for the vNV-Heap library
├── esp32c3_build_benchmarks.sh   # Build image for one specific or all benchmarks
├── esp32c3_run_benchmarks.sh     # Build and run one specific or all benchmarks.
├── notebooks_generate_plots.sh   # Generate the plots using the existing Jupyter notebooks
└── notebooks_start_server.sh     # Start the graphical Jupyter Notebook server. This can be used for example to choose select different raw data to be used for the plots
```

## Step-by-Step Instructions

*Note*: The following instructions require the Docker development container introduced in the [Getting Started Guide](#getting-started-guide).

### Run the Test Suite in a VM

``` bash
(.venv) USER@vnvheapae:~/vnv_heap/scripts$ ./desktop_run_testsuite.sh # 2min
```

This command build and runs the vNV-Heap library test suite.

This allows you to validate out claims regarding completeness of the
implementation, functionality, and features.

### Run Evaluation in a VM

``` bash
(.venv) USER@vnvheapae:~/vnv_heap/scripts$ ./desktop_run_benchmarks.sh # 7min
```

This command build and runs the vNV-Heap library and runs the evaluation
in a VM. Because the VM does not replicate the performance
characteristics, the numbers do not match out evaluation. If you have
the hardware available, please follow the instructions in the following
section. Otherwise, skip the following section and continue with "Plot
Evaluation Results".

This allows you to validate our claims regarding the completeness of the
evaluation.

### Run Evaluation on Hardware (Informative)

*This section is informative if you do not have similar hardware available locally. If desired we can provide you access to our hardware upon request.*

All the latency measurements used for evaluations require
*Espressif's ESP32-C3* microcontroller connected over *SPI* to a
*Fujitsu MB85RS64V FRAM* module. Reproducing the values from these
evaluations cannot be achieved inside the virtual machine for the
artifact evaluation, since the exact hardware setup is required to carry
out the evaluations.

Follow the next steps to run benchmarks on the target device:

1. Connect the FRAM chip to the ESP32-C3 as follows:
    - SCK: Pin 6
    - MISO: Pin 2
    - MOSI: Pin 7
    - CS: Pin 1
2. Connect the ESP32-C3 with your machine.
3. Check the path to the connected ESP32-C3. If this differs from `/dev/ttyUSB0` update `serial_port` in `zephyr/vnv_heap_auto_benchmark/record_benchmark.py`.
4. Check the baud rate of the connected ESP32-C3. If this differs from `115200` update `baud_rate` in `zephyr/vnv_heap_auto_benchmark/record_benchmark.py`.
5. If your Docker development container is currently running, stop it. This is required, as the development container does not support hot plugging.
6. Start the development container by running the `docker-run` script.
7. Run the `esp32c3_run_benchmarks.sh` and select the benchmark you want to run. Note that running the benchmarks takes a long time (especially for the queue and the key-value store)! For all four benchmarks this might take up to one day. To reduce this time, you might reduce the amount of repetitions (`VNV_HEAP_REPETITIONS`) in `scripts/esp32c3_run_benchmarks.sh` and reduce the iteration count (`ITERATION_COUNT`) in both `vnv_heap/src/benchmarks/applications/key_value_store/runner.rs` and `vnv_heap/src/benchmarks/applications/queue/runner.rs`.

The resulting measurements are automatically saved to a *.json* file, which can be used for further analysis or for plotting.

### Plot Evaluation Results

To plot measured data (saved as *.json* files) and therefore reproduce
Figures 3-7, use the Jupyter Notebooks stored in `evaluation/`.

1. First, start the local Jupyter server in the Docker container:

   ``` bash
   (.venv) USER@vnvheapae:~/vnv_heap/scripts$ ./notebooks_start_server.sh 
   ```

2. Then, open the displayed URL (e.g.
   `http://localhost:8888/lab?token=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`)
   in a browser. You can use your browser of your host system for that.
   If you are working on a remote server system, SSH to the remote
   machine with port forwarding using `ssh -L 8888:localhost:8888 $REMOTE_HOST`. Thereafter, you can use your Laptop's browser to
   navigate to the displayed URL.

3. Now, open the `*.ipynb` notebook file (double click in the sidebar) to
   validate our claims regarding the evaluation results:
   - Fig. 3: Bottom of `get_ref.ipynb`
   - Fig. 4: Bottom of `queue.ipynb`
   - Fig. 5: Bottom of `persist.ipynb`
   - Fig. 6 and Fig. 7: Bottom of `kvs.ipynb`

*Optionally*: If you want to plot other measurements than the one used
in the paper, update `file_name` (at the top of the notebook).

Now, run all cells to generate the new plot. The generated plot is also
saved in `evaluation/figures/` for your convenience.

If you want to generate the plots for all notebooks, run the following
script: `scripts/notebooks_generate_plots.sh`.
