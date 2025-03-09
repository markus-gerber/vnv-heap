use std::mem::size_of;

use crate::{
    benchmarks::{
        applications::key_value_store::{
            bench::{
                KeyValueStoreBenchmark,
                VNVHeapKeyValueStoreBenchmarkGeneralOptions,
            },
            page_wise::PagedKeyValueStoreImplementation,
            vnv_heap::VNVHeapKeyValueStoreImplementation,
        },
        common::multi_page::multi_page_calc_base_metadata_size,
    }, modules::object_management::DefaultObjectManagementModule, util::div_ceil, VNVConfig
};

use super::{super::super::*, calc_object_count_kvs_application, AccessType, KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES};

type A = LinkedListAllocatorModule;
type M = DefaultObjectManagementModule;
type N = NonResidentBuddyAllocatorModule<32>;

const ITERATION_COUNT: usize = 1_000;
const RAM_SIZE: usize = 120_000;
const OBJ_CNT: usize = 256;

fn get_access_types() -> [AccessType; 4] {
    [
        AccessType::Random,
        AccessType::Sequential,
        AccessType::Partitioned {
            partition_size: OBJ_CNT / 16,
            access_count: ITERATION_COUNT / 100,
            _curr_partition: 0, // only used for internal state
        },
        AccessType::Distributed {
            key_distribution: AccessType::key_distribution(
                |i: u32| -> f64 { (((i as f64) * 40.0) / (OBJ_CNT as f64)).sin().powi(20) + 0.1 },
                OBJ_CNT as u32,
            ),
        },
    ]
}

const PAGE_SIZES: [usize; 7] = [16, 32, 64, 128, 256, 512, 1024];

pub(crate) struct KVSBenchmarkRunner;

impl BenchmarkRunner for KVSBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        let access_types = get_access_types();
        if options.run_kvs_benchmarks {
            // ### paged ###
            iteration_count += access_types.len() * PAGE_SIZES.len();

            // ### vNV-Heap ###
            iteration_count += access_types.len();
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
            let max_dirty = {
                calc_object_count_kvs_application(OBJ_CNT).iter().zip(KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES).map(|(obj_count, obj_size)| {
                    obj_count * obj_size
                }).sum::<usize>() / 5
            };
            let access_types = get_access_types();
            {
                // ###### page-wise ######

                // GENERAL SETTINGS

                macro_rules! for_page_size_impl {
                    ($page_size: ident, $page_cnt: ident, $inner: expr, $value: expr) => {
                        {
                            static_assertions::const_assert_eq!(PAGE_SIZES.len(), $value);

                            seq_macro::seq!(I in 0..$value {
                                {
                                    const $page_size: usize = PAGE_SIZES[I];
                                    // make sure we have enough pages for all of the objects
                                    const $page_cnt: usize = div_ceil(RAM_SIZE, PAGE_SIZE);

                                    $inner
                                }
                            });
                        }

                    };
                }
                macro_rules! for_page_size {
                    ($page_size: ident, $page_cnt: ident, $inner: expr) => {
                        for_page_size_impl!(PAGE_SIZE, PAGE_CNT, $inner, 7);
                    };
                }

                for_page_size!(PAGE_SIZE, PAGE_CNT, {
                    // ETC RUNTIME CONSTANTS
                    let base_metadata_size: usize = multi_page_calc_base_metadata_size::<A, S>();

                    let mut storage = get_storage();
                    let mut pages = [[0u8; PAGE_SIZE]; PAGE_CNT];
                    
                    let page_cnt = {
                        let obj_cnts = calc_object_count_kvs_application(OBJ_CNT);
                        let obj_sizes = KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES;
                        let total_size = obj_cnts.iter().zip(obj_sizes).map(|(obj_count, obj_size)| {
                            obj_count * obj_size
                        }).sum();

                        div_ceil(total_size, PAGE_SIZE)
                    };

                    let metadata_size = 1*page_cnt + base_metadata_size;
                    let max_pages_dirty: usize = (max_dirty - metadata_size) / PAGE_SIZE;
                    assert!(max_pages_dirty > 0);

                    for access_type in access_types.iter().cloned() {
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

                        let bench = KeyValueStoreBenchmark::new(
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
                    
                    }
                
                });
            }

            {
                // ###### vNV-Heap ######

                // GENERAL SETTINGS
                // make sure we have enough space for all of the objects
                const VNV_HEAP_BUF_SIZE: usize = RAM_SIZE;

                // ETC RUNTIME CONSTANTS
                let vnv_heap_stack_size = size_of::<VNVHeap<A, N, M, S>>();

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
                let vnv_heap_max_dirty = max_dirty - vnv_heap_stack_size;

                for access_type in access_types.iter().cloned() {
                    handle_curr_iteration();

                    let storage = get_storage();

                    let heap = get_bench_heap(&mut buf, vnv_heap_max_dirty, storage);
                    let kvs_impl = VNVHeapKeyValueStoreImplementation::new(heap);
                    let bench = KeyValueStoreBenchmark::new(
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
