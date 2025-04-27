#!/usr/bin/env bash

echo "Note: This script will run all benchmarks except for the persist benchmark (as it requires the interrupt system of the esp32c3) on this machine."
echo "Note: This script is only meant to verify that the benchmarks are working and not for latency measurements."
echo "Note: For latency measurements, please use the \"esp32c3_run_benchmarks.sh\" script."
echo ""

read -p "Press Enter to continue..." PROMPT

cd $(dirname ${BASH_SOURCE[0]})/../../desktop/desktop_benchmark
cargo run
