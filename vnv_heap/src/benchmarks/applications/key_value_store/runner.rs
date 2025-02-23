use std::mem::size_of;

use crate::{
    benchmarks::{applications::key_value_store::{
        bench::{KeyValueStoreBenchmark, VNVHeapKeyValueStoreBenchmarkOptions}, page_wise::PagedKeyValueStoreImplementation, vnv_heap::VNVHeapKeyValueStoreImplementation
    }, common::multi_page::multi_page_calc_metadata_size},
    modules::{
        object_management::DefaultObjectManagementModule, persistent_storage::DummyStorageModule,
    },
    resident_object_manager::resident_object_metadata::ResidentObjectMetadata,
    VNVConfig,
};

use super::super::super::*;

type A = LinkedListAllocatorModule;
type M = DefaultObjectManagementModule;
type N = NonResidentBuddyAllocatorModule<32>;

const ITERATION_COUNT: usize = 1_000;
const OBJ_SIZE: usize = 256;
const OBJ_CNT: usize = 256;

// one 5th of the raw object size
const MAX_DIRTY: usize =  (OBJ_CNT * OBJ_SIZE)/5;

pub(crate) struct KVSBenchmarkRunner;

impl BenchmarkRunner for KVSBenchmarkRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_kvs_benchmarks {
            iteration_count += 2;
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
                handle_curr_iteration();

                // GENERAL SETTINGS
                // make sure we have enough pages for all of the objects
                const PAGE_SIZE: usize = 4096;
                const PAGE_CNT: usize = 2 + (OBJ_CNT * OBJ_SIZE) / PAGE_SIZE;

                // ETC RUNTIME CONSTANTS
                let metadata_size: usize = multi_page_calc_metadata_size::<PAGE_SIZE, PAGE_CNT, A, S>();
                let max_pages_dirty: usize = (MAX_DIRTY - metadata_size) / PAGE_SIZE;

                let mut storage = get_storage();
                let mut pages = [[0u8; PAGE_SIZE]; PAGE_CNT];
                let kvs_impl: PagedKeyValueStoreImplementation<'_, PAGE_SIZE, PAGE_CNT, LinkedListAllocatorModule, S> = PagedKeyValueStoreImplementation::new(&mut storage, A::new(), max_pages_dirty, &mut pages);
                let bench = KeyValueStoreBenchmark::new(
                    kvs_impl,
                    "kvs_paged",
                    VNVHeapKeyValueStoreBenchmarkOptions {
                        iterations: ITERATION_COUNT,
                        obj_cnt: OBJ_CNT,
                    },
                );
                bench.run_benchmark::<TIMER>(run_options);
            }

            {
                // ###### vNV-Heap ######
                handle_curr_iteration();


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

                let storage = get_storage();
                let heap = get_bench_heap(&mut buf, vnv_heap_max_dirty, storage);
                let kvs_impl = VNVHeapKeyValueStoreImplementation::new(heap);
                let bench = KeyValueStoreBenchmark::new(
                    kvs_impl,
                    "kvs",
                    VNVHeapKeyValueStoreBenchmarkOptions {
                        iterations: ITERATION_COUNT,
                        obj_cnt: OBJ_CNT,
                    },
                );
                bench.run_benchmark::<TIMER>(run_options);
            }
        }
    }
}
