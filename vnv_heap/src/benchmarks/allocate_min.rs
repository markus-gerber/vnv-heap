use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
    VNVHeap, VNVObject,
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, Timer};

#[derive(Serialize)]
pub struct AllocateMinBenchmarkOptions {
    object_size: usize,
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateMinBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    S: PersistentStorageModule,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, S>,

    #[allow(dead_code)]
    guard_objects:
        [VNVObject<'a, 'b, [u8; OBJ_SIZE], A, NonResidentBuddyAllocatorModule<16>, S>; 2],

    object_bucket_index: usize,
}

impl<'a, 'b: 'a, A: AllocatorModule, S: PersistentStorageModule, const OBJ_SIZE: usize>
    AllocateMinBenchmark<'a, 'b, A, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, S>) -> Self {
        let item = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();

        let item2 = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();
        let item3 = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();

        drop(item2);

        Self {
            heap: heap,
            guard_objects: [item, item3],
            object_bucket_index: OBJ_SIZE.next_power_of_two().trailing_zeros() as usize,
        }
    }
}

impl<'a, 'b: 'a, A: AllocatorModule, S: PersistentStorageModule, const OBJ_SIZE: usize>
    Benchmark<AllocateMinBenchmarkOptions> for AllocateMinBenchmark<'a, 'b, A, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_min"
    }

    #[inline]
    fn prepare_next_iteration(&mut self) {
        let heap_inner = self.heap.get_inner().borrow_mut();

        assert!(
            !heap_inner.get_non_resident_allocator().get_free_list()[self.object_bucket_index]
                .is_empty()
        );
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let timer = T::start();

        let item = black_box(self.heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE])).unwrap();
        let res = timer.stop();

        drop(item);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> &AllocateMinBenchmarkOptions {
        &AllocateMinBenchmarkOptions {
            object_size: OBJ_SIZE,
        }
    }
}
