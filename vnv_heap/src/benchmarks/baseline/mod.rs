
// only share the baseline model in this directory
// internal name BALE: BAseLinE model
mod model;

// import benchmarks
mod baseline_get_min;
mod baseline_get_max;
mod baseline_get_min_max;
mod baseline_get_max_min;

pub use baseline_get_min::*;
pub use baseline_get_max::*;
pub use baseline_get_min_max::*;
pub use baseline_get_max_min::*;
use model::MemoryManager;

pub use super::*;

type A = LinkedListAllocatorModule;

const BUCKET_SIZE: usize = 1024 + size_of::<A>();
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

pub(super) struct BaselineBenchmarkRunner;

impl BenchmarkRunner for BaselineBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_baseline_get_benchmarks {
            iteration_count += 4 * STEP_COUNT;
        }
        if options.run_baseline_allocate_benchmarks {
//            iteration_count += 2 * STEP_COUNT;
        }
        if options.run_baseline_deallocate_benchmarks {
//            iteration_count += 2 * STEP_COUNT;
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
        if options.run_baseline_get_benchmarks {
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineGetMinBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineGetMinBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineGetMaxMinBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineGetMaxMinBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineGetMinMaxBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineGetMinMaxBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineGetMaxBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineGetMaxBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
        }
    }

}


#[derive(Serialize)]
pub struct ModuleOptionsBaseline {
    allocator: &'static str,
}

impl ModuleOptionsBaseline {
    pub fn new<A: AllocatorModule>(
    ) -> Self {
        Self {
            allocator: type_name::<A>(),
        }
    }
}
