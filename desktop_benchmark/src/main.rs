use std::time::Instant;

use vnv_heap::{
    benchmarks::{
        run_all_benchmarks, BenchmarkRunOptions, DummyPersistTrigger,
        RunAllBenchmarkOptions, Timer,
    },
    modules::persistent_storage::FilePersistentStorageModule,
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
    /*
    use env_logger::{Builder, Env};
    Builder::from_env(Env::default())
        .filter_level(log::LevelFilter::Trace)
        .format_module_path(false)
        .init();*/

    run_all_benchmarks::<DesktopTimer, DummyPersistTrigger, FilePersistentStorageModule, _>(
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
        RunAllBenchmarkOptions {
            run_persist_latency_worst_case: true,
            ..Default::default()
        },
        get_storage,
        || 0,
    );
}

fn get_storage() -> FilePersistentStorageModule {
    FilePersistentStorageModule::new("test.data".into(), 4096 * 8).unwrap()
}
