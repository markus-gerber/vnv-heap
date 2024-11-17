mod persistent_storage_read;
mod persistent_storage_write;

pub use persistent_storage_read::*;
pub use persistent_storage_write::*;

pub use super::*;

const STEP_SIZE: usize = 32;
const MIN_OBJ_SIZE: usize = 0;
const MAX_OBJ_SIZE: usize = 1024;

const STEP_COUNT: usize = (MAX_OBJ_SIZE - MIN_OBJ_SIZE) / STEP_SIZE + 1;

macro_rules! for_obj_size_impl {
    ($index: ident, $inner: expr, $value: expr) => {
        static_assertions::const_assert_eq!($value, STEP_COUNT);
        seq_macro::seq!($index in 0..$value {
            {
                $inner
            }
        });
    };
}

macro_rules! for_obj_size {
    ($index: ident, $inner: expr) => {
        for_obj_size_impl!($index, $inner, 33);
    };
}

pub(super) struct StorageBenchmarkRunner;

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
        S: PersistentStorageModule + 'static,
        F: Fn() -> S,
        G: FnMut()
    >(
        run_options: &mut BenchmarkRunOptions,
        options: &RunAllBenchmarkOptions,
        get_storage: &F,
        handle_curr_iteration: &mut G
    ) {
        if options.run_persistent_storage_benchmarks {
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut storage_module = get_storage();

                let bench: PersistentStorageReadBenchmark<S, SIZE> = PersistentStorageReadBenchmark::new(&mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut storage_module = get_storage();

                let bench: PersistentStorageWriteBenchmark<S, SIZE> = PersistentStorageWriteBenchmark::new(&mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            });
        }
    
        if options.run_long_persistent_storage_benchmarks {
            {
                handle_curr_iteration();
                const SIZE: usize = 4096 * 2;
                let mut storage_module = get_storage();

                let bench: PersistentStorageReadBenchmark<S, SIZE> = PersistentStorageReadBenchmark::new(&mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            }
        
            {
                handle_curr_iteration();
                const SIZE: usize = 4096 * 2;
                let mut storage_module = get_storage();

                let bench: PersistentStorageWriteBenchmark<S, SIZE> = PersistentStorageWriteBenchmark::new(&mut storage_module);
                bench.run_benchmark::<TIMER>(run_options);
            }
        
        }
    }

}