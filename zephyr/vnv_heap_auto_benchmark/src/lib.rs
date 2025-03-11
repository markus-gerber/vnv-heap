extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;
extern crate zephyr_sys;

use spi_fram_storage::MB85RS4MTFramStorageModule;
use vnv_heap::benchmarks::{
    PersistTrigger, BenchmarkRunOptions, Timer, run_all_benchmarks, RunAllBenchmarkOptions
};
use vnv_heap::modules::persistent_storage::SlicedStorageModule;
use core::mem::{MaybeUninit, size_of};
use std::option_env;

extern "C" {
    pub fn helper_k_cycle_get_32() -> u32;
    pub fn helper_sys_clock_hw_cycles_per_sec() -> u32;
    pub fn helper_k_uptime_get() -> i64;
    pub fn helper_irq_lock() -> u64;
    pub fn helper_irq_unlock(key: u64);
    pub fn Cache_Invalidate_ICache_All();
}

#[no_mangle]
pub extern "C" fn rust_main() {
    let mut time: i64 = unsafe { helper_k_uptime_get() };

    {
        // make sure that the stack size was configured
        static_assertions::const_assert!(option_env!("CONFIG_MAIN_STACK_SIZE").is_some());
    }

    run_all_benchmarks::<
        ZephyrTimer,
        ZephyrPersistTrigger,
        SlicedStorageModule::<SLICE_SIZE, MB85RS4MTFramStorageModule>,
        _
    >(
        BenchmarkRunOptions {
            cold_start: 0,
            machine_name: "esp32c3",
            repetitions: 100,
            result_buffer: &mut [0; 100],
        },
        RunAllBenchmarkOptions {
            run_allocate_benchmarks: option_env!("VNV_HEAP_RUN_ALLOCATE_BENCHMARKS").is_some(),
            run_deallocate_benchmarks: option_env!("VNV_HEAP_RUN_DEALLOCATE_BENCHMARKS").is_some(),
            run_get_benchmarks: option_env!("VNV_HEAP_RUN_GET_BENCHMARKS").is_some(),
            run_baseline_allocate_benchmarks: option_env!("VNV_HEAP_RUN_BASELINE_ALLOCATE_BENCHMARKS").is_some(),
            run_baseline_deallocate_benchmarks: option_env!("VNV_HEAP_RUN_BASELINE_DEALLOCATE_BENCHMARKS").is_some(),
            run_baseline_get_benchmarks: option_env!("VNV_HEAP_RUN_BASELINE_GET_BENCHMARKS").is_some(),
            run_persistent_storage_benchmarks: option_env!("VNV_HEAP_RUN_PERSISTENT_STORAGE_BENCHMARKS").is_some(),
            run_long_persistent_storage_benchmarks: option_env!("VNV_HEAP_RUN_LONG_PERSISTENT_STORAGE_BENCHMARKS").is_some(),
            run_dirty_size_persist_latency: option_env!("VNV_HEAP_RUN_DIRTY_SIZE_PERSIST_LATENCY").is_some(),
            run_buffer_size_persist_latency: option_env!("VNV_HEAP_RUN_BUFFER_SIZE_PERSIST_LATENCY").is_some(),
            run_queue_benchmarks: option_env!("VNV_HEAP_RUN_QUEUE_BENCHMARKS").is_some(),
            run_kvs_benchmarks: option_env!("VNV_HEAP_RUN_KVS_BENCHMARKS").is_some(),
            run_locked_wcet_benchmarks: option_env!("VNV_HEAP_RUN_LOCKED_WCET_BENCHMARKS").is_some()
        },
        get_storage,
        || {
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            let res = unsafe { helper_k_cycle_get_32() };
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            res
        }
    );

    time = unsafe { helper_k_uptime_get() } - time; 

    let secs: i64 = (time / 1000) % 60;
    let mins: i64 = (time / (1000 * 60)) % 60;
    let hours: i64 = time / (1000 * 60 * 60);

    println!("[BENCH-STATUS] Finished in {}h {}m {}s", hours, mins, secs);
}

struct ZephyrTimer {
    start_time: u32,
    irq_key: u64
}

impl Timer for ZephyrTimer {

    fn get_ticks_per_ms() -> u32 {
        (unsafe { helper_sys_clock_hw_cycles_per_sec() }) / 1000
    }

    #[inline]
    fn start() -> Self {
        let irq_key = unsafe { helper_irq_lock() };
        unsafe { Cache_Invalidate_ICache_All(); }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let obj = Self {
            start_time: unsafe { helper_k_cycle_get_32() },
            irq_key
        };
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        obj
    }

    #[inline]
    fn stop(self) -> u32 {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let end_time = unsafe { helper_k_cycle_get_32() };
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        unsafe { helper_irq_unlock(self.irq_key) };

        let delta = end_time - self.start_time;

        delta
    }
}


static mut PERSIST_FUNCTION: Option<fn()> = None;

extern "C" fn persist_trigger_wrapper(_timer_id: *mut zephyr_sys::raw::k_timer) {
    unsafe {
        let irq_key = helper_irq_lock();
        Cache_Invalidate_ICache_All();

        (PERSIST_FUNCTION.unwrap())();

        helper_irq_unlock(irq_key);
    }
}

struct ZephyrPersistTrigger {
    timer: zephyr_sys::raw::k_timer
}

impl PersistTrigger for ZephyrPersistTrigger {
    fn new(function: fn()) -> Self {
        unsafe {
            if PERSIST_FUNCTION.is_some() {
                panic!("concurrency is not allowed!");
            }
            PERSIST_FUNCTION = Some(function);
        };

        // c-like initialization of timer struct
        let mut timer: MaybeUninit<zephyr_sys::raw::k_timer> = MaybeUninit::uninit();
        unsafe {
            zephyr_sys::raw::k_timer_init(timer.as_mut_ptr(), Some(persist_trigger_wrapper), None);
        }

        Self {
            timer: unsafe { timer.assume_init() }
        }
    }

    fn start_persist_trigger(&mut self) {
        // note: if the benchmark freezes you may want to update these timers
        unsafe {
            zephyr_sys::raw::z_impl_k_timer_start(&mut self.timer, zephyr_sys::raw::k_timeout_t { 
                ticks: (helper_sys_clock_hw_cycles_per_sec() as i64) / 10_000_000
            }, zephyr_sys::raw::k_timeout_t {
                ticks: (helper_sys_clock_hw_cycles_per_sec() as i64) / 10_000_000
            });
        }
    }

    fn stop_persist_trigger(&mut self) {
        unsafe {
            zephyr_sys::raw::z_impl_k_timer_stop(&mut self.timer);
        }
    }
}

impl Drop for ZephyrPersistTrigger {
    fn drop(&mut self) {
        unsafe {
            PERSIST_FUNCTION = None;
        };
    }
}

const SLICE_SIZE: usize = 4;

fn get_storage() -> SlicedStorageModule::<SLICE_SIZE, MB85RS4MTFramStorageModule> {
    let inner_storage = unsafe { MB85RS4MTFramStorageModule::new() }.unwrap();

    SlicedStorageModule::new(inner_storage)
}
