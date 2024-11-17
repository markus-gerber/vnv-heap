use std::time::Instant;

use crate::{
    benchmarks::{run_all_benchmarks, BenchmarkRunOptions, RunAllBenchmarkOptions, Timer},
    modules::persistent_storage::FilePersistentStorageModule,
};

use super::get_test_storage;

#[test]
fn test_benchmarks() {
    run_all_benchmarks::<
        DesktopTimer,
        FilePersistentStorageModule,
        _
    >(
        BenchmarkRunOptions {
            cold_start: 0,
            machine_name: "desktop",
            repetitions: 10,
            result_buffer: &mut [0; 10],
        },
        RunAllBenchmarkOptions::all(),
        get_storage
    );
}

fn get_storage() -> FilePersistentStorageModule {
    get_test_storage("test.data", 4096 * 4)
}

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
