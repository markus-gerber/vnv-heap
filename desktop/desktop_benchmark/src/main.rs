/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::{thread, time::Instant};

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

    // avoid stack overflow
    let builder = thread::Builder::new().stack_size(20 * 1024 * 1024);
        let handler = builder.spawn(|| {
            run_all_benchmarks::<DesktopTimer, DummyPersistTrigger, FilePersistentStorageModule, _>(
                BenchmarkRunOptions {
                    cold_start: 0,
                    machine_name: "desktop",
                    repetitions: 5,
                    result_buffer: &mut [0; 5],
                },
                // RunAllBenchmarkOptions::all_except_persist(),
                RunAllBenchmarkOptions {
                    run_allocate_benchmarks: true,
                    run_deallocate_benchmarks: true,
                    run_get_benchmarks: true,
                    run_baseline_allocate_benchmarks: true,
                    run_baseline_deallocate_benchmarks: true,
                    run_baseline_get_benchmarks: true,
                    run_persistent_storage_benchmarks: true,
                    run_long_persistent_storage_benchmarks: true,
                    run_dirty_size_persist_latency: false, // NOT SUPPORTED ON DESKTOP!
                    run_buffer_size_persist_latency: false, // NOT SUPPORTED ON DESKTOP!
                    run_queue_benchmarks: true,
                    run_kvs_benchmarks: true,
                    run_locked_wcet_benchmarks: true,
                },
                get_storage,
                || 0,
            );
        

    }).unwrap();
    handler.join().unwrap();
}

fn get_storage() -> FilePersistentStorageModule {
    FilePersistentStorageModule::new("test.data".into(), 512 * 1024).unwrap()
}
