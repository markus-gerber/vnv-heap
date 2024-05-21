extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;

use spi_fram_storage::SpiFramStorageModule;
use vnv_heap::benchmarks::{
    AllocateMaxBenchmark, AllocateMinBenchmark, Benchmark, BenchmarkRunOptions, Timer,
};
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
    },
    VNVConfig, VNVHeap,
};

extern "C" {
    pub fn helper_k_cycle_get_32() -> u32;
    pub fn helper_sys_clock_hw_cycles_per_sec() -> u32;
}

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Trace);

    seq_macro::seq!(I in 1..20 {
        {
            const SIZE: usize = I * 8;
            let mut buf = [0u8; 256];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let bench: AllocateMinBenchmark<LinkedListAllocatorModule, SpiFramStorageModule, SIZE> = AllocateMinBenchmark::new(&heap);
            bench.run_benchmark::<ZephyrTimer>(BenchmarkRunOptions {
                cold_start: 0,
                machine_name: "esp32c3",
                repetitions: 250,
                result_buffer: &mut [0; 250]
            });
        }
    });

    seq_macro::seq!(I in 1..20 {
        {
            const SIZE: usize = I * 8;
            let mut buf = [0u8; 256];
            let bench: AllocateMaxBenchmark<LinkedListAllocatorModule, SpiFramStorageModule, fn(&mut [u8], usize) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, SpiFramStorageModule>, SIZE> = AllocateMaxBenchmark::new(get_bench_heap, &mut buf);
            bench.run_benchmark::<ZephyrTimer>(BenchmarkRunOptions {
                cold_start: 0,
                machine_name: "esp32c3",
                repetitions: 250,
                result_buffer: &mut [0; 250]
            });
        }
    });

    println!("[BENCH-STATUS] Finished")
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
) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, SpiFramStorageModule> {
    let storage = unsafe { SpiFramStorageModule::new() }.unwrap();
    let config = VNVConfig {
        max_dirty_bytes: max_dirty,
    };

    VNVHeap::new(buf, storage, config).unwrap()
}
