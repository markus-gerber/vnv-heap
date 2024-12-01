use std::{thread, time::Instant};

use crate::{
    benchmarks::{run_all_benchmarks, BenchmarkRunOptions, DummyPersistTrigger, RunAllBenchmarkOptions, Timer},
    modules::persistent_storage::FilePersistentStorageModule,
};

use super::get_test_storage;

#[test]
fn test_benchmarks() {
    // avoid stack overflow
    let builder = thread::Builder::new().stack_size(20 * 1024 * 1024);
        let handler = builder.spawn(|| {
            run_all_benchmarks::<
            DesktopTimer,
            DummyPersistTrigger,
            FilePersistentStorageModule,
            _
        >(
            BenchmarkRunOptions {
                cold_start: 0,
                machine_name: "desktop",
                repetitions: 10,
                result_buffer: &mut [0; 10],
            },
            RunAllBenchmarkOptions::microbenchmarks(),
            get_storage,
            || {
                panic!("not implemented");
            }
        );

    }).unwrap();
    handler.join().unwrap();

}

fn get_storage() -> FilePersistentStorageModule {
    get_test_storage("test.data", 4096 * 8)
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
