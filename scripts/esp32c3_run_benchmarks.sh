#!/usr/bin/env bash

SCRIPT_ACTION=${SCRIPT_ACTION:-run}

cd $(dirname ${BASH_SOURCE[0]})/../zephyr/vnv_heap_auto_benchmark

echo "Available Benchmarks:"
echo "(1) Get reference"
echo "(2) Queue"
echo "(3) Persist"
echo "(4) Key-Value Store"
echo "(5) ALL BENCHMARKS"

read -p "Select a benchmark to ${SCRIPT_ACTION} (1-5): " benchmark_choice

while [[ ! "$benchmark_choice" =~ ^[1-5]$ ]]; do
    echo "Invalid choice. Please select a number between 1 and 5."
    read -p "Select a benchmark to ${SCRIPT_ACTION} (1-5): " benchmark_choice
done

echo ""


case $benchmark_choice in
    1)
        RUN_GET_BENCHMARK=1
        RUN_QUEUE_BENCHMARK=0
        RUN_PERSIST_BENCHMARK=0
        RUN_KVS_BENCHMARK=0
        ;;
    2)
        RUN_GET_BENCHMARK=0
        RUN_QUEUE_BENCHMARK=1
        RUN_PERSIST_BENCHMARK=0
        RUN_KVS_BENCHMARK=0
        ;;
    3)

        RUN_GET_BENCHMARK=0
        RUN_QUEUE_BENCHMARK=0
        RUN_PERSIST_BENCHMARK=1
        RUN_KVS_BENCHMARK=0
        ;;
    4)
        RUN_GET_BENCHMARK=0
        RUN_QUEUE_BENCHMARK=0
        RUN_PERSIST_BENCHMARK=0
        RUN_KVS_BENCHMARK=1
        ;;
    5)
        RUN_GET_BENCHMARK=1
        RUN_QUEUE_BENCHMARK=1
        RUN_PERSIST_BENCHMARK=1
        RUN_KVS_BENCHMARK=1
        ;;
esac

if [[ "$RUN_GET_BENCHMARK" == "1" ]]; then
    echo "Running get reference benchmark..."
    CONFIG_MAIN_STACK_SIZE=150000 VNV_HEAP_REPETITIONS=3 VNV_HEAP_RUN_GET_BENCHMARKS=1 VNV_HEAP_RUN_BASELINE_GET_BENCHMARKS=1 VNV_HEAP_RUN_PERSISTENT_STORAGE_BENCHMARKS=1 ./run_benchmark.sh "get"
fi

if [[ "$RUN_QUEUE_BENCHMARK" == "1" ]]; then
    echo "Running queue benchmark..."
    CONFIG_MAIN_STACK_SIZE=180000 VNV_HEAP_REPETITIONS=3 VNV_HEAP_RUN_QUEUE_BENCHMARKS=1 ./run_benchmark.sh "queue"
fi

if [[ "$RUN_PERSIST_BENCHMARK" == "1" ]]; then
    echo "Running persist benchmark..."
    CONFIG_MAIN_STACK_SIZE=150000 VNV_HEAP_REPETITIONS=3 VNV_HEAP_RUN_DIRTY_SIZE_PERSIST_LATENCY=1 VNV_HEAP_RUN_BUFFER_SIZE_PERSIST_LATENCY=1 VNV_HEAP_RUN_LOCKED_WCET_BENCHMARKS=1 VNV_HEAP_RUN_PERSISTENT_STORAGE_BENCHMARKS=1 ./run_benchmark.sh "persist"
fi

if [[ "$RUN_KVS_BENCHMARK" == "1" ]]; then
    echo "Running kvs benchmark..."
    CONFIG_MAIN_STACK_SIZE=215000 VNV_HEAP_REPETITIONS=3 VNV_HEAP_RUN_KVS_BENCHMARKS=1 ./run_benchmark.sh "kvs"
fi
