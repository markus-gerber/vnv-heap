use std::mem::size_of;

use crate::{
    benchmarks::{
        applications::key_value_store::{
            bench::{
                KeyValueStoreBenchmark, KeyValueStoreDiverseBenchmark,
                VNVHeapKeyValueStoreBenchmarkGeneralOptions,
            },
            page_wise::PagedKeyValueStoreImplementation,
            vnv_heap::VNVHeapKeyValueStoreImplementation,
        },
        common::multi_page::multi_page_calc_metadata_size,
    },
    modules::{
        object_management::DefaultObjectManagementModule, persistent_storage::DummyStorageModule,
    },
    resident_object_manager::resident_object_metadata::ResidentObjectMetadata,
    VNVConfig,
};

use super::{super::super::*, AccessType};

type A = LinkedListAllocatorModule;
type M = DefaultObjectManagementModule;
type N = NonResidentBuddyAllocatorModule<32>;

const ITERATION_COUNT: usize = 1_000;
const OBJ_SIZE: usize = 256;
const OBJ_CNT: usize = 256;

// one 5th of the raw object size
const MAX_DIRTY: usize = (OBJ_CNT * OBJ_SIZE) / 5;

const ACCESS_TYPES: [AccessType; 3] = [
    AccessType::Random,
    AccessType::Sequential,
    AccessType::Partitioned {
        partition_size: OBJ_CNT / 16,
        access_count: ITERATION_COUNT / 100,
        curr_partition: 0, // only used for internal state
    },
];

const PAGE_SIZES: [usize; 5] = [256, 512, 1024, 2048, 4096];

pub(crate) struct KVSBenchmarkRunner;

impl BenchmarkRunner for KVSBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_kvs_benchmarks {
            // ### paged ###
            iteration_count += 2 * ACCESS_TYPES.len() * PAGE_SIZES.len();

            // ### vNV-Heap ###
            iteration_count += 2 * ACCESS_TYPES.len();
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
        if options.run_kvs_benchmarks {
            {
                // ###### page-wise ######

                // GENERAL SETTINGS

                macro_rules! for_page_size_impl {
                    ($page_size: ident, $page_cnt: ident, $inner: expr, $value: expr) => {
                        {
                            static_assertions::const_assert_eq!(PAGE_SIZES.len(), 5);

                            seq_macro::seq!(I in 0..$value {
                                {
                                    const $page_size: usize = PAGE_SIZES[I];
                                    // make sure we have enough pages for all of the objects
                                    const $page_cnt: usize = 2 + (OBJ_CNT * OBJ_SIZE) / PAGE_SIZE;

                                    $inner
                                }
                            });
                        }

                    };
                }
                macro_rules! for_page_size {
                    ($page_size: ident, $page_cnt: ident, $inner: expr) => {
                        for_page_size_impl!(PAGE_SIZE, PAGE_CNT, $inner, 5);
                    };
                }

                for_page_size!(PAGE_SIZE, PAGE_CNT, {
                    // ETC RUNTIME CONSTANTS
                    let metadata_size: usize =
                        multi_page_calc_metadata_size::<PAGE_SIZE, PAGE_CNT, A, S>();
                    let max_pages_dirty: usize = (MAX_DIRTY - metadata_size) / PAGE_SIZE;

                    let mut storage = get_storage();
                    let mut pages = [[0u8; PAGE_SIZE]; PAGE_CNT];

                    for bench_type in 0..2 {
                        for &access_type in ACCESS_TYPES.iter() {
                            handle_curr_iteration();

                            let kvs_impl: PagedKeyValueStoreImplementation<
                                '_,
                                PAGE_SIZE,
                                PAGE_CNT,
                                LinkedListAllocatorModule,
                                S,
                            > = PagedKeyValueStoreImplementation::new(
                                &mut storage,
                                A::new(),
                                max_pages_dirty,
                                &mut pages,
                            );

                            if bench_type == 0 {
                                let bench: KeyValueStoreBenchmark<OBJ_SIZE, _, _, _> =
                                    KeyValueStoreBenchmark::new(
                                        kvs_impl,
                                        "kvs_paged",
                                        VNVHeapKeyValueStoreBenchmarkGeneralOptions {
                                            iterations: ITERATION_COUNT,
                                            object_count: OBJ_CNT,
                                            access_type,
                                            kvs_options: PagedKVSOptions {
                                                page_cnt: PAGE_CNT,
                                                page_size: PAGE_SIZE,
                                                max_pages_dirty,
                                                metadata_size,
                                            },
                                        },
                                    );
                                bench.run_benchmark::<TIMER>(run_options);
                            } else if bench_type == 1 {
                                let bench = KeyValueStoreDiverseBenchmark::new(
                                    kvs_impl,
                                    "kvs_paged_diverse",
                                    VNVHeapKeyValueStoreBenchmarkGeneralOptions {
                                        iterations: ITERATION_COUNT,
                                        object_count: OBJ_CNT,
                                        access_type,
                                        kvs_options: PagedKVSOptions {
                                            page_cnt: PAGE_CNT,
                                            page_size: PAGE_SIZE,
                                            max_pages_dirty,
                                            metadata_size,
                                        },
                                    },
                                );
                                bench.run_benchmark::<TIMER>(run_options);
                            } else {
                                panic!("Invalid bench type");
                            }
                        }
                    }
                });
            }

            {
                // ###### vNV-Heap ######

                // ETC CONSTANTS
                const VNV_HEAP_CUTOFF: usize =
                    VNVHeap::<'_, A, N, M, DummyStorageModule>::get_layout_info().cutoff_size;

                // GENERAL SETTINGS
                // make sure we have enough space for all of the objects
                const VNV_HEAP_BUF_SIZE: usize = OBJ_CNT
                    * (OBJ_SIZE + size_of::<ResidentObjectMetadata>())
                    + VNV_HEAP_CUTOFF
                    + 1024;

                // ETC RUNTIME CONSTANTS
                let vnv_heap_stack_size = size_of::<VNVHeap<A, N, M, S>>();
                let vnv_heap_max_dirty = MAX_DIRTY - vnv_heap_stack_size;

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

                for bench_type in 0..2 {
                    for &access_type in ACCESS_TYPES.iter() {
                        handle_curr_iteration();

                        let storage = get_storage();

                        let heap = get_bench_heap(&mut buf, vnv_heap_max_dirty, storage);
                        let kvs_impl = VNVHeapKeyValueStoreImplementation::new(heap);
                        if bench_type == 0 {
                            let bench: KeyValueStoreBenchmark<OBJ_SIZE, _, _, _> =
                                KeyValueStoreBenchmark::new(
                                    kvs_impl,
                                    "kvs",
                                    VNVHeapKeyValueStoreBenchmarkGeneralOptions {
                                        iterations: ITERATION_COUNT,
                                        object_count: OBJ_CNT,
                                        access_type,
                                        kvs_options: VNVHeapKVSOptions {
                                            max_dirty: vnv_heap_max_dirty,
                                        },
                                    },
                                );
                            bench.run_benchmark::<TIMER>(run_options);
                        } else if bench_type == 1 {
                            let bench = KeyValueStoreDiverseBenchmark::new(
                                kvs_impl,
                                "kvs_diverse",
                                VNVHeapKeyValueStoreBenchmarkGeneralOptions {
                                    iterations: ITERATION_COUNT,
                                    object_count: OBJ_CNT,
                                    access_type,
                                    kvs_options: VNVHeapKVSOptions {
                                        max_dirty: vnv_heap_max_dirty,
                                    },
                                },
                            );
                            bench.run_benchmark::<TIMER>(run_options);
                        } else {
                            panic!("Invalid bench type");
                        }
                    }
                }
            }
        }
    }
}

#[derive(Serialize, Clone)]
struct PagedKVSOptions {
    page_cnt: usize,
    page_size: usize,
    max_pages_dirty: usize,
    metadata_size: usize,
}

#[derive(Serialize, Clone)]
struct VNVHeapKVSOptions {
    max_dirty: usize,
}
