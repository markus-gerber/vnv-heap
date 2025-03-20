/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use core::any::type_name;

#[cfg(not(test))]
use std::io::stdout;

use persist_latency::DirtySizePersistLatencyRunner;
use serde::Serialize;

mod persist_latency;
pub use persist_latency::*;

mod microbenchmarks;
use microbenchmarks::*;

mod locked_wcet;
use locked_wcet::*;

pub(crate) mod common;

mod applications;
use applications::*;

use crate::{
    modules::{
        allocator::{AllocatorModule, LinkedListAllocatorModule}, nonresident_allocator::{NonResidentAllocatorModule, NonResidentBuddyAllocatorModule}, persistent_storage::PersistentStorageModule
    }, VNVHeap
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
    pub run_dirty_size_persist_latency: bool,
    pub run_buffer_size_persist_latency: bool,
    pub run_queue_benchmarks: bool,
    pub run_kvs_benchmarks: bool,
    pub run_locked_wcet_benchmarks: bool
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
            run_dirty_size_persist_latency: false,
            run_buffer_size_persist_latency: false,
            run_queue_benchmarks: false,
            run_kvs_benchmarks: false,
            run_locked_wcet_benchmarks: false
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
            run_dirty_size_persist_latency: true,
            run_buffer_size_persist_latency: true,
            run_queue_benchmarks: true,
            run_kvs_benchmarks: true,
            run_locked_wcet_benchmarks: true
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
    pub fn applications() -> Self {
        Self {
            run_queue_benchmarks: true,
            run_kvs_benchmarks: true,
            ..Default::default()
        }
    }
    pub fn all_except_persist() -> Self {
        Self {
            run_dirty_size_persist_latency: false,
            run_buffer_size_persist_latency: false,
            ..Self::all()
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
        debug_assert!(*curr_iteration < iteration_count);

        let percentage = (100 * *curr_iteration) / (iteration_count);
        print!("[{}%] ", percentage);

        *curr_iteration += 1;
    }

    // the following if conditions are used so the compiler can optimize and ignore runners that are not used
    // without them the binary would always include all benchmarks, which will not fit on smaller devices
    if options.run_allocate_benchmarks || options.run_get_benchmarks || options.run_deallocate_benchmarks {
        iteration_count += ImplementationBenchmarkRunner::get_iteration_count(&options);
    }
    if options.run_baseline_allocate_benchmarks || options.run_baseline_deallocate_benchmarks || options.run_baseline_get_benchmarks {
        iteration_count += BaselineBenchmarkRunner::get_iteration_count(&options);
    }
    if options.run_persistent_storage_benchmarks || options.run_long_persistent_storage_benchmarks {
        iteration_count += StorageBenchmarkRunner::get_iteration_count(&options);
    }
    if options.run_dirty_size_persist_latency {
        iteration_count += DirtySizePersistLatencyRunner::get_iteration_count(&options);
    }
    if options.run_buffer_size_persist_latency {
        iteration_count += BufferSizePersistLatencyRunner::get_iteration_count(&options);
    }
    if options.run_queue_benchmarks {
        iteration_count += QueueBenchmarkRunner::get_iteration_count(&options);
    }
    if options.run_kvs_benchmarks {
        iteration_count += KVSBenchmarkRunner::get_iteration_count(&options);
    }
    if options.run_locked_wcet_benchmarks {
        iteration_count += LockedWCETRunner::get_iteration_count(&options);
    }
    if iteration_count == 0 {
        println!("WARNING: No benchmarks selected to run! Please activate at least one benchmark via RunAllBenchmarkOptions.");
        return;
    }

    let mut handle_it = || {
        handle_curr_iteration(&mut curr_iteration, iteration_count);
    };

    // run benchmarks
    if options.run_allocate_benchmarks || options.run_get_benchmarks || options.run_deallocate_benchmarks {
        ImplementationBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_baseline_allocate_benchmarks || options.run_baseline_deallocate_benchmarks || options.run_baseline_get_benchmarks {
        BaselineBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_persistent_storage_benchmarks || options.run_long_persistent_storage_benchmarks {
        StorageBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_dirty_size_persist_latency {
        DirtySizePersistLatencyRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_buffer_size_persist_latency {
        BufferSizePersistLatencyRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_queue_benchmarks {
        QueueBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_kvs_benchmarks {
        KVSBenchmarkRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    if options.run_locked_wcet_benchmarks {
        LockedWCETRunner::run::<TIMER, TRIGGER, S, F, _>(&mut run_options, &options, &get_storage, &mut handle_it, get_ticks.clone());
    }
    debug_assert_eq!(curr_iteration, iteration_count);
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
