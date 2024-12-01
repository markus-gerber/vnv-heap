// mod allocate_case1;
// mod allocate_max;
// mod allocate_min;
// mod deallocate_case1;
// mod deallocate_max;
// mod deallocate_min;
mod get_max;
mod get_max_2;
mod get_max_min;
mod get_min;
mod get_min_max;

// pub use allocate_case1::*;
// pub use allocate_max::*;
// pub use allocate_min::*;
// pub use deallocate_case1::*;
// pub use deallocate_max::*;
// pub use deallocate_min::*;
pub use get_max::*;
pub use get_max_2::*;
pub use get_max_min::*;
pub use get_min::*;
pub use get_min_max::*;

use crate::{
    calc_resident_buf_cutoff_size, modules::object_management::DefaultObjectManagementModule, resident_object_manager::resident_object_metadata::ResidentObjectMetadata, VNVConfig
};

use super::*;

const RESIDENT_CUTOFF_SIZE: usize = {
    if size_of::<usize>() == 8 {
        // desktop with File Storage Module
        96
    } else if size_of::<usize>() == 4 {
        // zephyr with SPI Fram Storage module
        52
    } else {
        panic!("uhhm");
    }
};

// NOTE: if you change one of these three variables
// you also have to update the value in the for_obj_size macro!
const BUF_SIZE: usize = 1 * 1024 + RESIDENT_CUTOFF_SIZE;
const STEP_SIZE: usize = 16;
const MIN_OBJ_SIZE: usize = 8;
const MIN_OBJ_SIZE_RANGE: usize = 16;

// additional cost of linked list allocator (holes)
const ADDITIONAL_ALLOCATOR_COST: usize = 2 * size_of::<usize>();

const MAX_OBJ_SIZE_RANGE: usize = {
    const METADATA: usize = size_of::<ResidentObjectMetadata>();

    const MAX_SIZE: usize = BUF_SIZE - METADATA - RESIDENT_CUTOFF_SIZE - ADDITIONAL_ALLOCATOR_COST;

    // ensure max size is multiple of step size
    (MAX_SIZE / STEP_SIZE) * STEP_SIZE
};
const MAX_OBJ_SIZE: usize = {
    const METADATA: usize = size_of::<ResidentObjectMetadata>();
    const MAX_SIZE: usize = BUF_SIZE - METADATA - RESIDENT_CUTOFF_SIZE;

    if MAX_SIZE % ADDITIONAL_ALLOCATOR_COST == 0 {
        MAX_SIZE
    } else {
        const MAX_SIZE: usize = BUF_SIZE - METADATA - RESIDENT_CUTOFF_SIZE - ADDITIONAL_ALLOCATOR_COST;
        if MAX_SIZE < MAX_OBJ_SIZE_RANGE {
            MAX_OBJ_SIZE_RANGE
        } else {
            MAX_SIZE
        }
    }
};


const STEP_COUNT: usize = (MAX_OBJ_SIZE_RANGE - MIN_OBJ_SIZE_RANGE) / STEP_SIZE + 1;

macro_rules! for_obj_size_impl {
    ($index: ident, $inner: expr, $value: expr) => {
        {
            static_assertions::const_assert_eq!($value, STEP_COUNT);
            if MIN_OBJ_SIZE != MIN_OBJ_SIZE_RANGE {
                const $index: usize = MIN_OBJ_SIZE;
                $inner
            }

            seq_macro::seq!(I in 0..$value {
                {
                    const $index: usize = {
                        let tmp = I * STEP_SIZE + MIN_OBJ_SIZE_RANGE;
                        assert!(tmp % size_of::<usize>() == 0);
                        tmp
                    };
                    $inner
                }
            });

            if MAX_OBJ_SIZE != MAX_OBJ_SIZE_RANGE {
                const $index: usize = MAX_OBJ_SIZE;
                $inner
            }
        }
    };
}

macro_rules! for_obj_size {
    ($index: ident, $inner: expr) => {
        // the third argument has to be equal to STEP_COUNT!

        // because of the size of the metadata
        // STEP_COUNT has a different value for different target platforms!
        #[cfg(target_pointer_width = "32")]
        for_obj_size_impl!($index, $inner, 62);

        #[cfg(target_pointer_width = "64")]
        for_obj_size_impl!($index, $inner, 60);
    };
}

pub(crate) struct ImplementationBenchmarkRunner;

impl BenchmarkRunner for ImplementationBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_allocate_benchmarks {
            // iteration_count += 3 * STEP_COUNT;
            // if MIN_OBJ_SIZE != MIN_OBJ_SIZE_RANGE {
            //     iteration_count += 1;
            // }
            // if MAX_OBJ_SIZE != MAX_OBJ_SIZE_RANGE {
            //     iteration_count += 1;
            // }
        }
        if options.run_deallocate_benchmarks {
            // iteration_count += 3 * STEP_COUNT;
            // if MIN_OBJ_SIZE != MIN_OBJ_SIZE_RANGE {
            //     iteration_count += 1;
            // }
            // if MAX_OBJ_SIZE != MAX_OBJ_SIZE_RANGE {
            //     iteration_count += 1;
            // }
        }
        if options.run_get_benchmarks {
            iteration_count += 5 * STEP_COUNT;
            if MIN_OBJ_SIZE != MIN_OBJ_SIZE_RANGE {
                iteration_count += 1;
            }
            if MAX_OBJ_SIZE != MAX_OBJ_SIZE_RANGE {
                iteration_count += 1;
            }
        }

        iteration_count
    }
    fn run<TIMER: Timer, TRIGGER: PersistTrigger, S: PersistentStorageModule + 'static, F: Fn() -> S, G: FnMut()>(
        run_options: &mut BenchmarkRunOptions,
        options: &RunAllBenchmarkOptions,
        get_storage: &F,
        handle_curr_iteration: &mut G,
        _get_ticks: GetCurrentTicks,
    ) {
        type A = LinkedListAllocatorModule;
        type M = DefaultObjectManagementModule;

        assert_eq!(
            RESIDENT_CUTOFF_SIZE,
            calc_resident_buf_cutoff_size::<A, S>(),
            "cutoff size has to match"
        );

        fn get_bench_heap<'a, S2: PersistentStorageModule + 'static>(
            buf: &'a mut [u8],
            max_dirty: usize,
            storage: S2,
        ) -> VNVHeap<
            'a,
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<19>,
            DefaultObjectManagementModule,
            S2,
        > {
            let config = VNVConfig {
                max_dirty_bytes: max_dirty,
            };

            let heap: VNVHeap<
                LinkedListAllocatorModule,
                NonResidentBuddyAllocatorModule<19>,
                DefaultObjectManagementModule,
                S2,
            > = VNVHeap::new(
                buf,
                storage,
                LinkedListAllocatorModule::new(),
                config,
                |_, _| {},
            )
            .unwrap();

            heap
        }

        // allocate benchmarks
        if options.run_allocate_benchmarks {
            // for_obj_size!(SIZE, {
            //     handle_curr_iteration();
            //     let mut buf = [0u8; BUF_SIZE];
            //     let res_size = buf.len();
            //     let heap = get_bench_heap(&mut buf, res_size, get_storage());
            //     let bench: AllocateMinBenchmark<A, M, S, SIZE> = AllocateMinBenchmark::new(&heap);
            //     bench.run_benchmark::<TIMER>(run_options);
            // });
            // for_obj_size!(SIZE, {
            //     handle_curr_iteration();
            //     const METADATA_SIZE: usize = get_resident_size::<()>();
            //     const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            //     let mut buf = [0u8; BUF_SIZE];
            //     let res_size = buf.len();
            //     let mut heap = get_bench_heap(&mut buf, res_size, get_storage());
            //     let bench = AllocateCase1Benchmark::<A, M, S, SIZE, BLOCKER_SIZE>::new(&mut heap);
            //     bench.run_benchmark::<TIMER>(run_options);
            // });
            // for_obj_size!(SIZE, {
            //     handle_curr_iteration();
            //     const METADATA_SIZE: usize = get_resident_size::<()>();
            //     const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            //     let mut buf = [0u8; BUF_SIZE];
            //     let res_size = buf.len();
            //     let mut heap = get_bench_heap(&mut buf, res_size, get_storage());
            //     let bench = AllocateMaxBenchmark::<A, M, S, SIZE, BLOCKER_SIZE>::new(&mut heap);
            //     bench.run_benchmark::<TIMER>(run_options);
            // });
        }
        // deallocate benchmarks
        if options.run_deallocate_benchmarks {
            // for_obj_size!(SIZE, {
            //     handle_curr_iteration();
            //     let mut buf = [0u8; BUF_SIZE];
            //     let res_size = buf.len();
            //     let heap = get_bench_heap(&mut buf, res_size, get_storage());
            //     let bench: DeallocateMinBenchmark<A, M, S, SIZE> =
            //         DeallocateMinBenchmark::new(&heap);
            //     bench.run_benchmark::<TIMER>(run_options);
            // });
            // for_obj_size!(SIZE, {
            //     handle_curr_iteration();
            //     let mut buf = [0u8; BUF_SIZE];
            //     let res_size = buf.len();
            //     let heap = get_bench_heap(&mut buf, res_size, get_storage());
            //     let bench: DeallocateCase1Benchmark<A, M, S, SIZE> =
            //         DeallocateCase1Benchmark::new(&heap);
            //     bench.run_benchmark::<TIMER>(run_options);
            // });
            // for_obj_size!(SIZE, {
            //     handle_curr_iteration();
            //     const METADATA_SIZE: usize = get_resident_size::<()>();
            //     const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

            //     let mut buf = [0u8; BUF_SIZE];
            //     let res_size = buf.len();
            //     let mut heap = get_bench_heap(&mut buf, res_size, get_storage());
            //     let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
            //     let bench = DeallocateMaxBenchmark::<A, M, S, SIZE, BLOCKER_SIZE>::new(
            //         &mut heap,
            //         start_res_size,
            //     );
            //     bench.run_benchmark::<TIMER>(run_options);
            // });
        }

        // get reference benchmarks
        if options.run_get_benchmarks {
            for_obj_size!(SIZE, {
                handle_curr_iteration();
                let mut buf = [0u8; BUF_SIZE];
                let res_size = buf.len();
                let heap = get_bench_heap(&mut buf, res_size, get_storage());
                let bench: GetMinBenchmark<A, NonResidentBuddyAllocatorModule<19>, M, SIZE> =
                    GetMinBenchmark::new(&heap);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(SIZE, {
                handle_curr_iteration();

                const METADATA_SIZE: usize = get_resident_size::<()>();
                const BLOCKER_SIZE: usize = BUF_SIZE - METADATA_SIZE - RESIDENT_CUTOFF_SIZE;

                let mut buf = [0u8; BUF_SIZE];
                let res_size = buf.len();
                let heap = get_bench_heap(&mut buf, res_size, get_storage());
                let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
                let bench: GetMaxBenchmark<
                    A,
                    NonResidentBuddyAllocatorModule<19>,
                    M,
                    SIZE,
                    BLOCKER_SIZE,
                > = GetMaxBenchmark::new(&heap, start_res_size);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(SIZE, {
                handle_curr_iteration();

                const RES: (usize, usize) = calc_obj_cnt_and_rem_size_get_max(SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE);
                const BLOCKER_CNT: usize = RES.0;
                const REM_SIZE: usize = RES.1;

                let mut buf = [0u8; BUF_SIZE];
                let res_size = buf.len();
                let heap = get_bench_heap(&mut buf, res_size, get_storage());
                let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
                let bench: GetMax2Benchmark<
                    A,
                    NonResidentBuddyAllocatorModule<19>,
                    M,
                    SIZE,
                    REM_SIZE
                > = GetMax2Benchmark::new(&heap, start_res_size, BLOCKER_CNT);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(SIZE, {
                handle_curr_iteration();

                let mut buf = [0u8; BUF_SIZE];
                let res_size = buf.len();
                let heap = get_bench_heap(&mut buf, res_size, get_storage());
                let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
                let bench: GetMaxMinBenchmark<A, NonResidentBuddyAllocatorModule<19>, M, S, SIZE> =
                    GetMaxMinBenchmark::new(&heap, start_res_size);
                bench.run_benchmark::<TIMER>(run_options);
            });
            for_obj_size!(SIZE, {
                handle_curr_iteration();

                let mut buf = [0u8; BUF_SIZE];
                let res_size = buf.len();
                let heap = get_bench_heap(&mut buf, res_size, get_storage());
                let start_res_size = res_size - RESIDENT_CUTOFF_SIZE;
                let bench: GetMinMaxBenchmark<A, NonResidentBuddyAllocatorModule<19>, M, S, SIZE> =
                    GetMinMaxBenchmark::new(&heap, start_res_size);
                bench.run_benchmark::<TIMER>(run_options);
            });
        }
    }
}
