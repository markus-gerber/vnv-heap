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

mod persistent_storage_read;
mod persistent_storage_write;

pub use persistent_storage_read::*;
pub use persistent_storage_write::*;

use super::*;

const STEP_SIZE: usize = 4;
const MIN_OBJ_SIZE: usize = 0;
const MAX_OBJ_SIZE: usize = 8 * 1024;

const STEP_COUNT: usize = (MAX_OBJ_SIZE - MIN_OBJ_SIZE) / STEP_SIZE + 1;

macro_rules! for_buffer {
    ($buffer_name: ident, $inner: expr) => {
        {
            let mut general_buffer: [u8; MAX_OBJ_SIZE] = [0; MAX_OBJ_SIZE];
            for i in 0..STEP_COUNT {
                let obj_size = i * STEP_SIZE + MIN_OBJ_SIZE;
                let $buffer_name = &mut general_buffer[0..obj_size];
                $inner
            }
        }
    };
}

pub(crate) struct StorageBenchmarkRunner;

impl BenchmarkRunner for StorageBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_persistent_storage_benchmarks {
            iteration_count += 2 * STEP_COUNT;
        }
        if options.run_long_persistent_storage_benchmarks {
            iteration_count += 2;
        }
        iteration_count
    }

    fn run<
        TIMER: Timer,
        TRIGGER: PersistTrigger,
        S: PersistentStorageModule + 'static,
        F: Fn() -> S,
        G: FnMut()
    >(
        run_options: &mut BenchmarkRunOptions,
        options: &RunAllBenchmarkOptions,
        get_storage: &F,
        handle_curr_iteration: &mut G,
        _get_ticks: GetCurrentTicks,
    ) {
        if options.run_persistent_storage_benchmarks {
            for_buffer!(buffer, {
                handle_curr_iteration();
                let mut storage_module = get_storage();

                let bench: PersistentStorageReadBenchmark<S> = PersistentStorageReadBenchmark::new(buffer, &mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_buffer!(buffer, {
                handle_curr_iteration();
                let mut storage_module = get_storage();

                let bench: PersistentStorageWriteBenchmark<S> = PersistentStorageWriteBenchmark::new(buffer, &mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            });
        }
    
        if options.run_long_persistent_storage_benchmarks {
            const SIZE: usize = 4096 * 4;
            let mut buffer = [0u8; SIZE];

            {
                handle_curr_iteration();
                let mut storage_module = get_storage();

                let bench: PersistentStorageReadBenchmark<S> = PersistentStorageReadBenchmark::new(&mut buffer, &mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            }
        
            {
                handle_curr_iteration();
                let mut storage_module = get_storage();

                let bench: PersistentStorageWriteBenchmark<S> = PersistentStorageWriteBenchmark::new(&mut buffer, &mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            }
        
        }
    }

}