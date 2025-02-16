
use super::super::common::single_page::MemoryManager;

// import benchmarks
mod baseline_get_min;
mod baseline_get_max;
mod baseline_get_min_max;
mod baseline_get_max_min;
mod baseline_allocate_min;
mod baseline_allocate_max;
mod baseline_allocate_min_max;
mod baseline_allocate_max_min;
mod baseline_deallocate_min;
mod baseline_deallocate_max;
mod baseline_deallocate_min_max;
mod baseline_deallocate_max_min;

pub use baseline_get_min::*;
pub use baseline_get_max::*;
pub use baseline_get_min_max::*;
pub use baseline_get_max_min::*;
pub use baseline_allocate_min::*;
pub use baseline_allocate_max::*;
pub use baseline_allocate_min_max::*;
pub use baseline_allocate_max_min::*;
pub use baseline_deallocate_min::*;
pub use baseline_deallocate_max::*;
pub use baseline_deallocate_min_max::*;
pub use baseline_deallocate_max_min::*;

use super::*;
use core::mem::size_of;

type A = LinkedListAllocatorModule;

const BUCKET_SIZE: usize = 1024 + size_of::<A>();
const STEP_SIZE: usize = 16;
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
        for_obj_size_impl!($index, $inner, 65);
    };
}

pub(crate) struct BaselineBenchmarkRunner;

impl BenchmarkRunner for BaselineBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_baseline_get_benchmarks {
            iteration_count += 4 * STEP_COUNT;
        }
        if options.run_baseline_allocate_benchmarks {
            iteration_count += 4 * STEP_COUNT;
        }
        if options.run_baseline_deallocate_benchmarks {
            iteration_count += 4 * STEP_COUNT;
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
        if options.run_baseline_allocate_benchmarks {
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineAllocateMinBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineAllocateMinBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineAllocateMaxMinBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineAllocateMaxMinBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineAllocateMinMaxBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineAllocateMinMaxBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineAllocateMaxBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineAllocateMaxBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
        }
        if options.run_baseline_deallocate_benchmarks {
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineDeallocateMinBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineDeallocateMinBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineDeallocateMaxMinBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineDeallocateMaxMinBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineDeallocateMinMaxBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineDeallocateMinMaxBenchmark::new(&mut memory_manager);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(I, {
                handle_curr_iteration();
                const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                let mut buffer = [0u8; BUCKET_SIZE];
                let mut storage = get_storage();
                let mut memory_manager = MemoryManager::new(&mut buffer, &mut storage, 2, LinkedListAllocatorModule::new);

                let bench: BaselineDeallocateMaxBenchmark<SIZE, BUCKET_SIZE, A, S> = BaselineDeallocateMaxBenchmark::new(&mut memory_manager);
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
