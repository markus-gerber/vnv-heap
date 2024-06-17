extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;
extern crate zephyr_sys;

use spi_fram_storage::SpiFramStorageModule;
use vnv_heap::benchmarks::{
    BenchmarkRunOptions, Timer, run_all_benchmarks, RunAllBenchmarkOptions
};
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule
    },
    VNVConfig, VNVHeap,
};

extern "C" {
    pub fn helper_k_cycle_get_32() -> u32;
    pub fn helper_sys_clock_hw_cycles_per_sec() -> u32;
    pub fn helper_k_uptime_get() -> i64;
}

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Trace);
    let mut time: i64 = unsafe { helper_k_uptime_get() };
    
    run_all_benchmarks::<
        ZephyrTimer,
        SpiFramStorageModule,
        DefaultObjectManagementModule,
        fn(
            &mut [u8],
            usize,
        ) -> VNVHeap<
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<16>,
            DefaultObjectManagementModule
        >,
    >(
        get_bench_heap,
        BenchmarkRunOptions {
            cold_start: 0,
            machine_name: "esp32c3",
            repetitions: 500,
            result_buffer: &mut [0; 500],
        },
        /*RunAllBenchmarkOptions {
            run_deallocate_benchmarks: true,
            run_persistent_storage_benchmarks: true,
            ..Default::default()
        },*/
        RunAllBenchmarkOptions::all()
    );

    time = unsafe { helper_k_uptime_get() } - time; 

    let secs: i64 = (time / 1000) % 60;
    let mins: i64 = (time / (1000 * 60)) % 60;
    let hours: i64 = time / (1000 * 60 * 60);

    println!("[BENCH-STATUS] Finished in {}h {}m {}s", hours, mins, secs);
}

struct ZephyrTimer {
    start_time: u32,
}

impl Timer for ZephyrTimer {
    #[inline]
    fn start() -> Self {
        Self {
            start_time: unsafe { helper_k_cycle_get_32() },
        }
    }

    #[inline]
    fn stop(self) -> u32 {
        let end_time = unsafe { helper_k_cycle_get_32() };

        let delta = end_time - self.start_time;
        let cycles_per_sec = unsafe { helper_sys_clock_hw_cycles_per_sec() };

        // avoiding overflow of (delta * b) by dividing
        // cycles_per_sec (without losing accuracy)
        let mut a = 1;
        let mut b = 1_000_000;
        while cycles_per_sec % (a * 10) == 0 && b > 1 {
            a *= 10;
            b /= 10;
        }

        (delta * b) / (cycles_per_sec / a)
    }
}

fn get_bench_heap(
    buf: &mut [u8],
    max_dirty: usize,
) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, DefaultObjectManagementModule> {
    let storage = unsafe { SpiFramStorageModule::new() }.unwrap();
    let config = VNVConfig {
        max_dirty_bytes: max_dirty,
    };

    VNVHeap::new(buf, storage, LinkedListAllocatorModule::new(), config, |_, _| {}).unwrap()
}
