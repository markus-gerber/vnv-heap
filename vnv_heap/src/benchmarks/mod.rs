use core::{any::type_name, mem::size_of};

#[cfg(not(test))]
use std::io::stdout;

use persist_latency::PersistLatencyRunner;
use serde::Serialize;

mod persist_latency;
pub use persist_latency::*;

mod microbenchmarks;
use microbenchmarks::*;

pub(crate) mod baseline;


use crate::{
    modules::{
        allocator::{AllocatorModule, LinkedListAllocatorModule}, nonresident_allocator::{NonResidentAllocatorModule, NonResidentBuddyAllocatorModule}, persistent_storage::PersistentStorageModule
    }, resident_object_manager::get_total_resident_size, VNVHeap
};

pub struct RunAllBenchmarkOptions {
    pub run_allocate_benchmarks: bool,
    pub run_deallocate_benchmarks: bool,
    pub run_get_benchmarks: bool,
    pub run_baseline_allocate_benchmarks: bool,
    pub run_baseline_deallocate_benchmarks: bool,
    pub run_baseline_get_benchmarks: bool,
    pub run_persistent_storage_benchmarks: bool,
    pub run_long_persistent_storage_benchmarks: bool,
    pub run_persist_latency_worst_case: bool
}

impl Default for RunAllBenchmarkOptions {
    fn default() -> Self {
        Self {
            run_allocate_benchmarks: false,
            run_deallocate_benchmarks: false,
            run_get_benchmarks: false,
            run_baseline_allocate_benchmarks: false,
            run_baseline_deallocate_benchmarks: false,
            run_baseline_get_benchmarks: false,
            run_persistent_storage_benchmarks: false,
            run_long_persistent_storage_benchmarks: false,

            run_persist_latency_worst_case: false,
        }
    }
}

impl RunAllBenchmarkOptions {
    pub fn all() -> Self {
        Self {
            run_allocate_benchmarks: true,
            run_deallocate_benchmarks: true,
            run_get_benchmarks: true,
            run_baseline_allocate_benchmarks: true,
            run_baseline_deallocate_benchmarks: true,
            run_baseline_get_benchmarks: true,
            run_persistent_storage_benchmarks: true,
            run_long_persistent_storage_benchmarks: true,
            run_persist_latency_worst_case: true
        }
    }
    pub fn microbenchmarks() -> Self {
        Self {
            run_allocate_benchmarks: true,
            run_deallocate_benchmarks: true,
            run_get_benchmarks: true,
            run_baseline_allocate_benchmarks: true,
            run_baseline_deallocate_benchmarks: true,
            run_baseline_get_benchmarks: true,
            run_persistent_storage_benchmarks: true,
            run_long_persistent_storage_benchmarks: true,
            ..Default::default()
        }
    }
}

pub fn run_all_benchmarks<
    TIMER: Timer,
    TRIGGER: PersistTrigger,
    S: PersistentStorageModule + 'static,
    F: Fn() -> S
>(
    mut run_options: BenchmarkRunOptions,
    options: RunAllBenchmarkOptions,
    get_storage: F,
    get_ticks: GetCurrentTicks,
) {
    let mut curr_iteration = 0usize;
    let mut iteration_count = 0;

    fn handle_curr_iteration(curr_iteration: &mut usize, iteration_count: usize) {
        let percentage = (100 * *curr_iteration) / (iteration_count);
        print!("[{}%] ", percentage);

        *curr_iteration += 1;
    }

    iteration_count += ImplementationBenchmarkRunner::get_iteration_count(&options);
    iteration_count += BaselineBenchmarkRunner::get_iteration_count(&options);
    iteration_count += StorageBenchmarkRunner::get_iteration_count(&options);
    iteration_count += PersistLatencyRunner::get_iteration_count(&options);
    let mut handle_it = || {
        handle_curr_iteration(&mut curr_iteration, iteration_count);
    };

    // run benchmarks
    ImplementationBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    BaselineBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    StorageBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    PersistLatencyRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    println!("")
    
}

pub(self) trait BenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize;

    fn run<
        TIMER: Timer,
        TRIGGER: PersistTrigger,
        S: PersistentStorageModule + 'static,
        F: Fn() -> S,
        G: FnMut(),
    >(
        run_options: &mut BenchmarkRunOptions,
        options: &RunAllBenchmarkOptions,
        get_storage: &F,
        handle_curr_iteration: &mut G,
        get_ticks: GetCurrentTicks,
    );

}

pub trait Benchmark<O: Serialize> {
    fn get_name(&self) -> &'static str;

    fn get_bench_options(&self) -> O;

    fn execute<T: Timer>(&mut self) -> u32;

    #[inline(never)]
    fn run_benchmark<T: Timer>(mut self, options: &mut BenchmarkRunOptions) -> BenchmarkRunResult
    where
        Self: Sized,
    {
        assert_eq!(options.repetitions as usize, options.result_buffer.len());

        print!("Running Benchmark \"{}\" with options ", self.get_name());

        #[cfg(not(test))]
        serde_json::to_writer(stdout(), &self.get_bench_options()).unwrap();
        println!();

        for _ in 0..options.cold_start {
            self.execute::<T>();
        }

        for i in 0..options.result_buffer.len() {
            let res = self.execute::<T>();
            options.result_buffer[i] = res;
        }

        
        
        print!("[BENCH-INFO] ");

        #[cfg(not(test))]
        {
            let run_info = BenchmarkRunInfo {
                bench_name: self.get_name(),
                bench_options: &self.get_bench_options(),
                machine_name: options.machine_name,
                cold_start: options.cold_start,
                repetitions: options.repetitions,
                ticks_per_ms: T::get_ticks_per_ms(),
                data: &options.result_buffer,
            };
            serde_json::to_writer(stdout(), &run_info).unwrap();    
        }
        println!("");

        let res = BenchmarkRunResult::from_buffer(&options.result_buffer);
        println!(
            "-> Finished {}: mean={}, min={}, max={}",
            self.get_name(),
            res.mean_latency,
            res.min_latency,
            res.max_latency
        );
        println!();

        res
    }
}

pub struct BenchmarkRunOptions<'a> {
    pub repetitions: u32,
    pub result_buffer: &'a mut [u32],

    pub cold_start: u32,

    pub machine_name: &'static str,
}

#[derive(Serialize)]
pub struct ModuleOptions {
    allocator: &'static str,
    non_resident_allocator: &'static str,
}

impl ModuleOptions {
    pub fn new<A: AllocatorModule, N: NonResidentAllocatorModule>(
    ) -> Self {
        Self {
            allocator: type_name::<A>(),
            non_resident_allocator: type_name::<N>(),
        }
    }
}

#[derive(Serialize)]
pub struct BenchmarkRunInfo<'a, O: Serialize> {
    bench_name: &'static str,
    bench_options: &'a O,
    machine_name: &'static str,
    cold_start: u32,
    repetitions: u32,
    ticks_per_ms: u32,
    data: &'a [u32],
}

pub struct BenchmarkRunResult {
    pub mean_latency: u32,
    pub min_latency: u32,
    pub max_latency: u32,
}

impl BenchmarkRunResult {
    fn from_buffer(buffer: &[u32]) -> Self {
        Self {
            mean_latency: buffer.iter().map(|x| *x).sum::<u32>() / (buffer.len() as u32),
            min_latency: buffer.iter().min().map(|x| *x).unwrap(),
            max_latency: buffer.iter().max().map(|x| *x).unwrap(),
        }
    }
}

pub trait Timer {
    fn get_ticks_per_ms() -> u32;

    fn start() -> Self;

    fn stop(self) -> u32;
}
