extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;
extern crate zephyr_sys;

use spi_fram_storage::MB85RS4MTFramStorageModule;
use vnv_heap::benchmarks::{
    BenchmarkRunOptions, Timer, run_all_benchmarks, RunAllBenchmarkOptions
};

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
    zephyr_logger::init(log::LevelFilter::Trace);
    let mut time: i64 = unsafe { helper_k_uptime_get() };
/*
    {
        let layout_info = VNVHeap::<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, DefaultObjectManagementModule, MB85RS4MTFramStorageModule>::get_layout_info();
        println!("layout_info: {:?}", layout_info);
    }
*/
    run_all_benchmarks::<
        ZephyrTimer,
        MB85RS4MTFramStorageModule,
        _
    >(
        BenchmarkRunOptions {
            cold_start: 0,
            machine_name: "esp32c3",
            repetitions: 10,
            result_buffer: &mut [0; 10],
        },
        /*RunAllBenchmarkOptions {
            run_deallocate_benchmarks: true,
        //    run_persistent_storage_benchmarks: true,
            ..Default::default()
        },*/
        RunAllBenchmarkOptions::all(),
        get_storage
    );

    time = unsafe { helper_k_uptime_get() } - time; 

    let secs: i64 = (time / 1000) % 60;
    let mins: i64 = (time / (1000 * 60)) % 60;
    let hours: i64 = time / (1000 * 60 * 60);

    println!("[BENCH-STATUS] Finished in {}h {}m {}s", hours, mins, secs);
}

#[allow(unused)]
fn measure_timer() {
    let mut x = [0u32; 1000];
    for _ in 0..100 { 
        for i in 0..1000 {
            let timer = ZephyrTimer::start();
            x[i] = timer.stop();
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
        for i in 0..1000 {
            println!("{}", x[i]);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // dirty way to stop the CPU
    assert!(false);
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

fn get_storage() -> MB85RS4MTFramStorageModule {
    unsafe { MB85RS4MTFramStorageModule::new() }.unwrap()
}
