use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
    VNVHeap, VNVObject,
};
use core::hint::black_box;
use std::{cmp::max, mem::size_of};
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct DeallocateMinBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct DeallocateMinBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    S: PersistentStorageModule,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, S>,

    #[allow(dead_code)]
    item_guard: Option<VNVObject<'a, 'b, [u8; OBJ_SIZE], A, NonResidentBuddyAllocatorModule<16>, S>>,

    object_bucket_index: usize,
}

impl<'a, 'b: 'a, A: AllocatorModule, S: PersistentStorageModule, const OBJ_SIZE: usize>
    DeallocateMinBenchmark<'a, 'b, A, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, S>) -> Self {
        let object_bucket_index = (max(OBJ_SIZE, size_of::<usize>())).next_power_of_two().trailing_zeros() as usize;
        
        let mut item_guard = None;
        let empty = {
            let heap_inner = heap.get_inner().borrow_mut();
            heap_inner.get_non_resident_allocator().get_free_list()[object_bucket_index].is_empty()
        };
        if empty {
            item_guard = Some(heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap())
        }

        Self {
            heap: heap,
            item_guard,
            object_bucket_index,
        }
    }
}

impl<'a, 'b: 'a, A: AllocatorModule, S: PersistentStorageModule, const OBJ_SIZE: usize>
    Benchmark<DeallocateMinBenchmarkOptions> for DeallocateMinBenchmark<'a, 'b, A, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "deallocate_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let item = self.heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();

        {
            let heap_inner = self.heap.get_inner().borrow_mut();
            assert!(
                heap_inner.get_non_resident_allocator().get_free_list()[self.object_bucket_index]
                    .is_empty(),
                "Make sure that if you deallocate, no two buckets can get merged"
            );
        }

        let timer = T::start();

        black_box(drop(item));

        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> DeallocateMinBenchmarkOptions {
        DeallocateMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>, S>()
        }
    }
}
