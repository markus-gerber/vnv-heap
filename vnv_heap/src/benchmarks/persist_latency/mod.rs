// dirty size runner
mod dsize_runner;

// buffer size runner
mod bsize_runner;

mod worst_case;
pub(super) use dsize_runner::*;
pub(super) use bsize_runner::*;

use std::sync::atomic::AtomicBool;

use serde::Serialize;

#[cfg(not(test))]
use std::io::stdout;

#[cfg(not(test))]
use crate::benchmarks::BenchmarkRunInfo;

use crate::{
    benchmarks::BenchmarkRunResult,
    modules::object_management::DefaultObjectManagementModule,
    vnv_persist_all, VNVConfig, VNVHeap,
};

use super::{
    microbenchmarks::{
        LinkedListAllocatorModule, NonResidentBuddyAllocatorModule, PersistentStorageModule,
    },
    BenchmarkRunOptions, Timer,
};

pub type GetCurrentTicks = fn() -> u32;

struct Helper {
    result_list: Option<Vec<u32>>,
    result_index: usize,
    get_ticks: GetCurrentTicks,
}

static BENCHMARK_STARTED: AtomicBool = AtomicBool::new(false);
static PERSIST_STARTED: AtomicBool = AtomicBool::new(false);
static FINISHED: AtomicBool = AtomicBool::new(false);
static mut RESULTS: Helper = Helper {
    result_list: None,
    result_index: 0,
    get_ticks: get_ticks_dummy,
};

fn get_ticks_dummy() -> u32 {
    0
}

fn get_persist_bench_heap<'a, S: PersistentStorageModule + 'static>(
    buf: &'a mut [u8],
    max_dirty: usize,
    storage: S,
) -> VNVHeap<
    'a,
    LinkedListAllocatorModule,
    NonResidentBuddyAllocatorModule<19>,
    DefaultObjectManagementModule,
    S,
> {
    let config = VNVConfig {
        max_dirty_bytes: max_dirty,
    };

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<19>,
        DefaultObjectManagementModule,
        S,
    > = VNVHeap::new(
        buf,
        storage,
        LinkedListAllocatorModule::new(),
        config,
        |_, _| {
            let helper = unsafe { &mut RESULTS };
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            let ticks = (helper.get_ticks)();
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

            if helper.result_index >= helper.result_list.as_mut().unwrap().len() {
                // finished
                FINISHED.store(true, std::sync::atomic::Ordering::SeqCst);
            } else {
                // store result
                helper.result_list.as_mut().unwrap()[helper.result_index] = ticks;
            }
        },
    )
    .unwrap();

    heap
}

trait PersistBenchmark<O: Serialize> {
    fn get_name(&self) -> &'static str;

    fn get_bench_options(&self) -> O;

    fn loop_until_finished(&mut self, finished: &AtomicBool);

    #[inline(never)]
    fn run_benchmark<T: Timer, TRIGGER: PersistTrigger>(
        mut self,
        options: &mut BenchmarkRunOptions,
        get_ticks: GetCurrentTicks,
        trigger: &mut TRIGGER,
    ) where
        Self: Sized,
    {
        if BENCHMARK_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            panic!("benchmark already running")
        }

        unsafe {
            RESULTS = Helper {
                get_ticks,
                result_index: 0,
                result_list: Some(vec![0; options.result_buffer.len()]),
            };
        }
        FINISHED.store(false, std::sync::atomic::Ordering::SeqCst);

        assert_eq!(options.repetitions as usize, options.result_buffer.len());

        print!("Running Benchmark \"{}\" with options ", self.get_name());

        #[cfg(not(test))]
        serde_json::to_writer(stdout(), &self.get_bench_options()).unwrap();
        println!();

        trigger.start_persist_trigger();
        self.loop_until_finished(&FINISHED);
        trigger.stop_persist_trigger();

        assert!(FINISHED.load(std::sync::atomic::Ordering::SeqCst));
        
        unsafe {
            let res = RESULTS.result_list.as_mut().unwrap();
            assert_eq!(res.len(), options.result_buffer.len());
            for i in 0..res.len() {
                options.result_buffer[i] = res[i];
            }
        }
        print!("[BENCH-INFO] ");

        #[cfg(not(test))]
        {
            let run_info = BenchmarkRunInfo {
                bench_name: self.get_name(),
                bench_options: &self.get_bench_options(),
                machine_name: options.machine_name,
                cold_start: 0,
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

        unsafe {
            RESULTS = Helper {
                get_ticks: get_ticks_dummy,
                result_index: 0,
                result_list: None,
            }
        };

        BENCHMARK_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn trigger_persist() {
    if PERSIST_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    let helper = unsafe { &mut RESULTS };
    if helper.result_index >= helper.result_list.as_mut().unwrap().len() {
        // finished
        FINISHED.store(true, std::sync::atomic::Ordering::SeqCst);
    } else {
        // store result
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let ticks = (helper.get_ticks)();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        unsafe { vnv_persist_all() };

        helper.result_list.as_mut().unwrap()[helper.result_index] -= ticks;
        helper.result_index += 1;
        
        if helper.result_index >= helper.result_list.as_mut().unwrap().len() {
            // finished
            FINISHED.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    PERSIST_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
}

pub trait PersistTrigger {
    fn new(function: fn()) -> Self;
    fn start_persist_trigger(&mut self);
    fn stop_persist_trigger(&mut self);

}

pub struct DummyPersistTrigger;

impl PersistTrigger for DummyPersistTrigger {
    fn new(_function: fn()) -> Self {
        panic!("dummy");
    }

    fn start_persist_trigger(&mut self) {
        panic!("dummy")
    }

    fn stop_persist_trigger(&mut self) {
        panic!("dummy")
    }
}
