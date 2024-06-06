use std::time::Instant;

use crate::{
    benchmarks::{run_all_benchmarks, BenchmarkRunOptions, RunAllBenchmarkOptions, Timer},
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::FilePersistentStorageModule,
    },
    VNVConfig, VNVHeap,
};

use super::get_test_heap;

#[test]
fn test_benchmarks() {
    run_all_benchmarks::<
        DesktopTimer,
        LinkedListAllocatorModule,
        FilePersistentStorageModule,
        fn(
            &mut [u8],
            usize,
        ) -> VNVHeap<
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<16>,
            FilePersistentStorageModule,
        >,
    >(
        get_bench_heap,
        BenchmarkRunOptions {
            cold_start: 0,
            machine_name: "desktop",
            repetitions: 10,
            result_buffer: &mut [0; 10],
        },
        RunAllBenchmarkOptions::all(),
    );
}

fn get_bench_heap(
    buf: &mut [u8],
    max_dirty: usize,
) -> VNVHeap<
    LinkedListAllocatorModule,
    NonResidentBuddyAllocatorModule<16>,
    FilePersistentStorageModule,
> {
    get_test_heap("test_benchmarks", 4096 * 4, buf, max_dirty)
}

struct DesktopTimer {
    start_time: Instant,
}

impl Timer for DesktopTimer {
    #[inline]
    fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    #[inline]
    fn stop(self) -> u32 {
        (Instant::now() - self.start_time).subsec_micros()
    }
}
