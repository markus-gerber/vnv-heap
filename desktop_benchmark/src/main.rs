use std::time::Instant;

// use env_logger::{Builder, Env};
use vnv_heap::{
    benchmarks::{
        run_all_benchmarks,
        BenchmarkRunOptions,
        RunAllBenchmarkOptions, Timer,
    },
    modules::{
        allocator::LinkedListAllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule, object_management::DefaultObjectManagementModule, persistent_storage::FilePersistentStorageModule
    },
    VNVConfig, VNVHeap,
};

struct DesktopTimer {
    start_time: Instant,
}

impl Timer for DesktopTimer {

    fn get_ticks_per_ms() -> u32 {
        1000
    }

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

fn main() {
    /*Builder::from_env(Env::default())
        .filter_level(log::LevelFilter::Trace)
        .format_module_path(false)
        .init();
*/
    run_all_benchmarks::<
        DesktopTimer,
        FilePersistentStorageModule,
        DefaultObjectManagementModule,
        fn(
            &mut [u8],
            usize,
        ) -> VNVHeap<
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<16>,
            DefaultObjectManagementModule,
            FilePersistentStorageModule
        >,
    >(
        get_bench_heap,
        BenchmarkRunOptions {
            cold_start: 0,
            machine_name: "desktop",
            repetitions: 5,
            result_buffer: &mut [0; 5],
        },
        //RunAllBenchmarkOptions::default(),
        /*RunAllBenchmarkOptions {
            run_persistent_storage_benchmarks: true,
            run_long_persistent_storage_benchmarks: true,
            ..Default::default()
        }*/
        RunAllBenchmarkOptions::all()
    );
}

fn get_bench_heap(
    buf: &mut [u8],
    max_dirty: usize,
) -> VNVHeap<
    LinkedListAllocatorModule,
    NonResidentBuddyAllocatorModule<16>,
    DefaultObjectManagementModule,
    FilePersistentStorageModule
> {
    let storage = FilePersistentStorageModule::new("test.data".to_string(), 4096 * 4).unwrap();
    let config = VNVConfig {
        max_dirty_bytes: max_dirty,
    };

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<16>,
        DefaultObjectManagementModule,
        FilePersistentStorageModule
    > = VNVHeap::new(buf, storage, LinkedListAllocatorModule::new(), config, |_, _| {}).unwrap();

    heap
}
