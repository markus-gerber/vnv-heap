use core::{any::type_name, mem::size_of};

#[cfg(not(test))]
use std::io::stdout;

use serde::Serialize;

mod allocate_max;
mod allocate_min;
mod deallocate_max;
mod deallocate_case1;
mod get_max;
mod get_min;
mod persistent_storage_read;
mod persistent_storage_write;
mod deallocate_min;
mod get_case1;
mod allocate_case1;

pub use allocate_max::*;
pub use allocate_min::*;
pub use deallocate_max::*;
pub use deallocate_case1::*;
pub use get_max::*;
pub use get_min::*;
pub use persistent_storage_read::*;
pub use persistent_storage_write::*;
pub use deallocate_min::*;
pub use get_case1::*;
pub use allocate_case1::*;


use crate::{
    modules::{
        allocator::{AllocatorModule, LinkedListAllocatorModule}, nonresident_allocator::{NonResidentAllocatorModule, NonResidentBuddyAllocatorModule}, object_management::ObjectManagementModule, persistent_storage::{PersistentStorageModule, SharedStorageReference}
    }, resident_object_manager::get_total_resident_size, vnv_heap::calc_resident_buf_cutoff_size, VNVHeap
};

pub struct RunAllBenchmarkOptions {
    pub run_allocate_benchmarks: bool,
    pub run_deallocate_benchmarks: bool,
    pub run_get_benchmarks: bool,
    pub run_persistent_storage_benchmarks: bool,
    pub run_long_persistent_storage_benchmarks: bool,
}

impl Default for RunAllBenchmarkOptions {
    fn default() -> Self {
        Self {
            run_allocate_benchmarks: false,
            run_deallocate_benchmarks: false,
            run_get_benchmarks: false,
            run_persistent_storage_benchmarks: false,
            run_long_persistent_storage_benchmarks: false,
        }
    }
}

impl RunAllBenchmarkOptions {
    pub fn all() -> Self {
        Self {
            run_allocate_benchmarks: true,
            run_deallocate_benchmarks: true,
            run_get_benchmarks: true,
            run_persistent_storage_benchmarks: true,
            run_long_persistent_storage_benchmarks: true,
        }
    }
}

pub fn run_all_benchmarks<
    TIMER: Timer,
    S: PersistentStorageModule + 'static,
    M: ObjectManagementModule,
    F: Fn(&mut [u8], usize) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, M, S>,
>(
    get_bench_heap: F,
    mut run_options: BenchmarkRunOptions,
    options: RunAllBenchmarkOptions,
) {
    type A = LinkedListAllocatorModule;

    const RESIDENT_CUTOFF_SIZE: usize = {
        if size_of::<usize>() == 8 {
            // desktop with File Storage Module
            112
        } else if size_of::<usize>() == 4 {
            // zephyr with SPI Fram Storage module
            60
        } else {
            panic!("uhhm");
        }
    };

    // NOTE: if you change one of these three variables
    // you also have to update the value in the for_obj_size macro!
    const BUF_SIZE: usize = 1024;
    const STEP_SIZE: usize = 32;
    const MIN_OBJ_SIZE: usize = 0;

    // additional cost of linked list allocator (holes)
    // const ADDITIONAL_ALLOCATOR_COST: usize = 16;
    const ADDITIONAL_ALLOCATOR_COST: usize = 0; // just for testing
    

    const MAX_OBJ_SIZE: usize = {
        const BIG_OBJ: usize = get_total_resident_size::<[u8; BUF_SIZE]>();
        const METADATA: usize = BIG_OBJ - BUF_SIZE;

        const MAX_SIZE: usize = BUF_SIZE - METADATA - RESIDENT_CUTOFF_SIZE - ADDITIONAL_ALLOCATOR_COST;

        // ensure max size is multiple of step size
        (MAX_SIZE / STEP_SIZE) * STEP_SIZE
    };

    const STEP_COUNT: usize = (MAX_OBJ_SIZE - MIN_OBJ_SIZE) / STEP_SIZE + 1;

    assert_eq!(RESIDENT_CUTOFF_SIZE, calc_resident_buf_cutoff_size::<A, S>(), "cutoff size has to match");

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
            // the third argument has to be equal to STEP_COUNT!

            // because of the size of the metadata
            // STEP_COUNT has a different value for different target platforms!
            #[cfg(target_pointer_width = "32")]
            for_obj_size_impl!($index, $inner, 30);

            #[cfg(target_pointer_width = "64")]
            for_obj_size_impl!($index, $inner, 27);
        };
    }

    let mut curr_iteration = 0usize;
    let mut iteration_count = 0;

    if options.run_allocate_benchmarks {
        iteration_count += 3 * STEP_COUNT;
    }
    if options.run_deallocate_benchmarks {
        iteration_count += 3 * STEP_COUNT;
    }
    if options.run_get_benchmarks {
        iteration_count += 3 * STEP_COUNT;
    }
    if options.run_persistent_storage_benchmarks {
        iteration_count += 2 * STEP_COUNT;
    }
    if options.run_long_persistent_storage_benchmarks {
        iteration_count += 2;
    }

    fn handle_curr_iteration(curr_iteration: &mut usize, iteration_count: usize) {
        let percentage = (100 * *curr_iteration) / (iteration_count);
        print!("[{}%] ", percentage);

        *curr_iteration += 1;
    }

    // allocate benchmarks
    if options.run_allocate_benchmarks {
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let bench: AllocateMinBenchmark<A, M, S, SIZE> = AllocateMinBenchmark::new(&heap);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            const METADATA_SIZE: usize = get_resident_size::<()>();
            const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let mut heap = get_bench_heap(&mut buf, res_size);
            let bench = AllocateCase1Benchmark::<A, M, S, SIZE, BLOCKER_SIZE>::new(&mut heap);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            const METADATA_SIZE: usize = get_resident_size::<()>();
            const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let mut heap = get_bench_heap(&mut buf, res_size);
            let bench = AllocateMaxBenchmark::<A, M, S, SIZE, BLOCKER_SIZE>::new(&mut heap);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
    }
    // deallocate benchmarks
    if options.run_deallocate_benchmarks {
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let bench: DeallocateMinBenchmark<A, M, S, SIZE> = DeallocateMinBenchmark::new(&heap);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let bench: DeallocateCase1Benchmark<A, M, S, SIZE> = DeallocateCase1Benchmark::new(&heap);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            const METADATA_SIZE: usize = get_resident_size::<()>();
            const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let mut heap = get_bench_heap(&mut buf, res_size);
            let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
            let bench = DeallocateMaxBenchmark::<A, M, S, SIZE, BLOCKER_SIZE>::new(&mut heap, start_res_size);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
    }

    // get reference benchmarks
    if options.run_get_benchmarks {
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let bench: GetMinBenchmark<A, NonResidentBuddyAllocatorModule<16>, M, SIZE> = GetMinBenchmark::new(&heap);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        /* 
        seq_macro::seq!(BLOCKERS_SIZE in 0..2 {
            {
                for_obj_size!(I, {
                    handle_curr_iteration(&mut curr_iteration, iteration_count);
                    // BLOCKERS_SIZE + 16 as blockers contain at least 16 bytes of metadata
                    // (even much more in reality)
                    const BLOCKERS_COUNT: usize = (BUF_SIZE / (BLOCKERS_SIZE + 16)) + 1;

                    const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
                    let mut buf = [0u8; BUF_SIZE];
                    let res_size = buf.len();
                    let heap = get_bench_heap(&mut buf, res_size);
                    let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
                    let bench: GetMax1Benchmark<A, NonResidentBuddyAllocatorModule<16>, M, SIZE, BLOCKERS_SIZE, BLOCKERS_COUNT> = GetMax1Benchmark::new(&heap, start_res_size);
                    bench.run_benchmark::<TIMER>(&mut run_options);
                });
            }
        });*/
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);

            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            const METADATA_SIZE: usize = get_resident_size::<()>();
            const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
            let bench: GetMax2Benchmark<A, NonResidentBuddyAllocatorModule<16>, M, SIZE, BLOCKER_SIZE> = GetMax2Benchmark::new(&heap, start_res_size);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;

            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();
            let heap = get_bench_heap(&mut buf, res_size);
            let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
            let bench: GetCase1Benchmark<A, NonResidentBuddyAllocatorModule<16>, M, S, SIZE> = GetCase1Benchmark::new(&heap, start_res_size);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
    }

    if options.run_persistent_storage_benchmarks {
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();

            // hacky way to get storage module
            let heap = get_bench_heap(&mut buf, res_size);
            let mut inner = heap.get_inner().borrow_mut();
            let storage_module = inner.get_storage_module();

            let bench: PersistentStorageReadBenchmark<SharedStorageReference, SIZE> = PersistentStorageReadBenchmark::new(storage_module);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
        for_obj_size!(I, {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = I * STEP_SIZE + MIN_OBJ_SIZE;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();

            // hacky way to get storage module
            let heap = get_bench_heap(&mut buf, res_size);
            let mut inner = heap.get_inner().borrow_mut();
            let storage_module = inner.get_storage_module();

            let bench: PersistentStorageWriteBenchmark<SharedStorageReference, SIZE> = PersistentStorageWriteBenchmark::new(storage_module);
            bench.run_benchmark::<TIMER>(&mut run_options);
        });
    }

    if options.run_long_persistent_storage_benchmarks {
        {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = 4096 * 2;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();

            // hacky way to get storage module
            let heap = get_bench_heap(&mut buf, res_size);
            let mut inner = heap.get_inner().borrow_mut();
            let storage_module = inner.get_storage_module();

            let bench: PersistentStorageReadBenchmark<SharedStorageReference, SIZE> = PersistentStorageReadBenchmark::new(storage_module);
            bench.run_benchmark::<TIMER>(&mut run_options);
        }
    
        {
            handle_curr_iteration(&mut curr_iteration, iteration_count);
            const SIZE: usize = 4096 * 2;
            let mut buf = [0u8; BUF_SIZE];
            let res_size = buf.len();

            // hacky way to get storage module
            let heap = get_bench_heap(&mut buf, res_size);
            let mut inner = heap.get_inner().borrow_mut();
            let storage_module = inner.get_storage_module();

            let bench: PersistentStorageWriteBenchmark<SharedStorageReference, SIZE> = PersistentStorageWriteBenchmark::new(storage_module);
            bench.run_benchmark::<TIMER>(&mut run_options);
        }
    
    }
}

pub trait Benchmark<O: Serialize> {
    fn get_name(&self) -> &'static str;

    fn get_bench_options(&self) -> O;

    fn execute<T: Timer>(&mut self) -> u32;

    #[inline(never)]
    fn run_benchmark<T: Timer>(mut self, options: &mut BenchmarkRunOptions) -> BenchmarkRunResult
    where
        Self: Sized,
    {
        assert_eq!(options.repetitions as usize, options.result_buffer.len());

        print!("Running Benchmark \"{}\" with options ", self.get_name());

        #[cfg(not(test))]
        serde_json::to_writer(stdout(), &self.get_bench_options()).unwrap();
        println!();

        for _ in 0..options.cold_start {
            self.execute::<T>();
        }

        for i in 0..options.result_buffer.len() {
            let res = self.execute::<T>();
            options.result_buffer[i] = res;
        }

        
        
        print!("[BENCH-INFO] ");

        #[cfg(not(test))]
        {
            let run_info = BenchmarkRunInfo {
                bench_name: self.get_name(),
                bench_options: &self.get_bench_options(),
                machine_name: options.machine_name,
                cold_start: options.cold_start,
                repetitions: options.repetitions,
                ticks_per_ms: T::get_ticks_per_ms(),
                data: &options.result_buffer,
            };
            serde_json::to_writer(stdout(), &run_info).unwrap();    
        }
        println!("");

        let res = BenchmarkRunResult::from_buffer(&options.result_buffer);
        println!(
            "-> Finished {}: mean={}, min={}, max={}",
            self.get_name(),
            res.mean_latency,
            res.min_latency,
            res.max_latency
        );
        println!();

        res
    }
}

pub struct BenchmarkRunOptions<'a> {
    pub repetitions: u32,
    pub result_buffer: &'a mut [u32],

    pub cold_start: u32,

    pub machine_name: &'static str,
}

#[derive(Serialize)]
pub struct ModuleOptions {
    allocator: &'static str,
    non_resident_allocator: &'static str,
}

impl ModuleOptions {
    pub fn new<A: AllocatorModule, N: NonResidentAllocatorModule>(
    ) -> Self {
        Self {
            allocator: type_name::<A>(),
            non_resident_allocator: type_name::<N>(),
        }
    }
}

#[derive(Serialize)]
pub struct BenchmarkRunInfo<'a, O: Serialize> {
    bench_name: &'static str,
    bench_options: &'a O,
    machine_name: &'static str,
    cold_start: u32,
    repetitions: u32,
    ticks_per_ms: u32,
    data: &'a [u32],
}

pub struct BenchmarkRunResult {
    pub mean_latency: u32,
    pub min_latency: u32,
    pub max_latency: u32,
}

impl BenchmarkRunResult {
    fn from_buffer(buffer: &[u32]) -> Self {
        Self {
            mean_latency: buffer.iter().map(|x| *x).sum::<u32>() / (buffer.len() as u32),
            min_latency: buffer.iter().min().map(|x| *x).unwrap(),
            max_latency: buffer.iter().max().map(|x| *x).unwrap(),
        }
    }
}

pub trait Timer {
    fn get_ticks_per_ms() -> u32;

    fn start() -> Self;

    fn stop(self) -> u32;
}
