use std::{
    array::from_fn,
    mem::{size_of, MaybeUninit},
};

use applications::event_queue::{
    implementation::EventQueueImplementationBenchmark, ram::EventQueueRAMBenchmark,
    storage::EventQueueStorageBenchmark,
};
use crate::{
    modules::{
        nonresident_allocator::{
            calc_non_resident_block_allocator_bit_list_size, NonResidentBlockAllocator,
        },
        object_management::DefaultObjectManagementModule,
        persistent_storage::{DummyStorageModule, TruncatedStorageModule},
    },
    vnv_list::{ListItemContainer, VNVList},
    VNVConfig,
};

use super::super::super::*;

const BLOCK_SIZE: usize = size_of::<ListItemContainer<[u8; OBJ_SIZE]>>();
const STORAGE_SIZE: usize = 2 * MAX_TOTAL_SIZE;
const BIT_LIST_SIZE: usize =
    calc_non_resident_block_allocator_bit_list_size(BLOCK_SIZE, STORAGE_SIZE);

type A = LinkedListAllocatorModule;
type M = DefaultObjectManagementModule;
type N = NonResidentBlockAllocator<BLOCK_SIZE, BIT_LIST_SIZE>;

const ITERATION_COUNT: usize = 10;
const OBJ_SIZE: usize = 256;

const RAM_LIMIT: usize = 4 * 1024;

const VNV_HEAP_RAM_OVERHEAD: usize = {
    size_of::<VNVList<'_, '_, [u8; OBJ_SIZE], A, N, M>>()
        + size_of::<VNVHeap<'_, A, N, M, DummyStorageModule>>()
        + VNVHeap::<'_, A, N, M, DummyStorageModule>::get_layout_info().persist_access_point_size
};
const VNV_HEAP_BUF_SIZE: usize = RAM_LIMIT - VNV_HEAP_RAM_OVERHEAD;

const STEP_SIZE: usize = 1;
const MIN_TOTAL_SIZE: usize = 0;
const MAX_TOTAL_SIZE: usize = 8 * 1024;
const MAX_OBJ_CNT: usize = {
    assert!(MAX_TOTAL_SIZE % OBJ_SIZE == 0);
    MAX_TOTAL_SIZE / OBJ_SIZE
};
const MIN_OBJ_CNT: usize = {
    assert!(MIN_TOTAL_SIZE % OBJ_SIZE == 0);
    MIN_TOTAL_SIZE / OBJ_SIZE
};

const STEP_COUNT: usize = (MAX_OBJ_CNT - MIN_OBJ_CNT) / STEP_SIZE + 1;

macro_rules! for_obj_cnt_impl {
    ($index: ident, $inner: expr, $value: expr) => {
        static_assertions::const_assert_eq!($value, STEP_COUNT);
        seq_macro::seq!(STEP_INDEX in 0..$value {
            {
                const $index: usize = STEP_INDEX * STEP_SIZE + MIN_OBJ_CNT;
                {
                    $inner
                }
            }
        });
    };
}

macro_rules! for_obj_cnt {
    ($index: ident, $inner: expr) => {
        for_obj_cnt_impl!($index, $inner, 33);
    };
}

pub(crate) struct EventQueueBenchmarkRunner;

impl BenchmarkRunner for EventQueueBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_event_queue_benchmarks {
            iteration_count += 3 * STEP_COUNT;
        }

        iteration_count
    }

    fn run<
        TIMER: Timer,
        TRIGGER: PersistTrigger,
        S: PersistentStorageModule + 'static,
        F: Fn() -> S,
        G: FnMut(),
    >(
        run_options: &mut BenchmarkRunOptions,
        options: &RunAllBenchmarkOptions,
        get_storage: &F,
        handle_curr_iteration: &mut G,
        _get_ticks: GetCurrentTicks,
    ) {
        if options.run_event_queue_benchmarks {
            {
                const MAX_BUF_SIZE: usize = MAX_OBJ_CNT + 2;
                let mut buffer: [MaybeUninit<[u8; OBJ_SIZE]>; MAX_BUF_SIZE] =
                    from_fn(|_| MaybeUninit::uninit());

                for_obj_cnt!(OBJ_CNT, {
                    handle_curr_iteration();

                    let bench: EventQueueRAMBenchmark<OBJ_SIZE> = EventQueueRAMBenchmark::new(
                        &mut buffer[0..(OBJ_CNT + 1)],
                        OBJ_CNT,
                        ITERATION_COUNT,
                    );
                    bench.run_benchmark::<TIMER>(run_options);
                });
            }

            for_obj_cnt!(OBJ_CNT, {
                handle_curr_iteration();

                let mut storage = get_storage();
                let bench: EventQueueStorageBenchmark<OBJ_SIZE, S> =
                    EventQueueStorageBenchmark::new(&mut storage, OBJ_CNT, ITERATION_COUNT);
                bench.run_benchmark::<TIMER>(run_options);
            });

            {
                fn get_bench_heap<'a, S2: PersistentStorageModule + 'static>(
                    buf: &'a mut [u8],
                    max_dirty: usize,
                    storage: S2,
                ) -> VNVHeap<'a, A, N, M, S2> {
                    let config = VNVConfig {
                        max_dirty_bytes: max_dirty,
                    };

                    let heap: VNVHeap<A, N, M, S2> = VNVHeap::new(
                        buf,
                        storage,
                        LinkedListAllocatorModule::new(),
                        config,
                        |_, _| {},
                    )
                    .unwrap();

                    heap
                }

                let mut buf = [0u8; VNV_HEAP_BUF_SIZE];

                for_obj_cnt!(OBJ_CNT, {
                    handle_curr_iteration();

                    let buf_len = buf.len();
                    let storage = get_storage();
                    let storage = TruncatedStorageModule::<STORAGE_SIZE, S>::new(storage);
                    let mut heap = get_bench_heap(&mut buf, buf_len, storage);

                    let bench: EventQueueImplementationBenchmark<A, N, M, OBJ_SIZE> =
                        EventQueueImplementationBenchmark::new(
                            &mut heap,
                            OBJ_CNT,
                            ITERATION_COUNT,
                            VNV_HEAP_BUF_SIZE,
                            VNV_HEAP_RAM_OVERHEAD,
                        );
                    bench.run_benchmark::<TIMER>(run_options);
                });
            }
        }
    }
}
