use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule, object_management::ObjectManagementModule
    },
    VNVHeap, VNVObject,
};
use core::hint::black_box;
use core::{cmp::max, mem::size_of};
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct AllocateMinBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateMinBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M>,

    #[allow(dead_code)]
    item_guard: Option<VNVObject<'a, 'b, [u8; OBJ_SIZE], A, NonResidentBuddyAllocatorModule<16>, M>>,

    object_bucket_index: usize,
}

impl<'a, 'b: 'a, A: AllocatorModule + 'static, M: ObjectManagementModule, const OBJ_SIZE: usize>
    AllocateMinBenchmark<'a, 'b, A, M, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M>) -> Self {
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
            heap,
            item_guard,
            object_bucket_index,
        }
    }
}

impl<'a, 'b: 'a, A: AllocatorModule, M: ObjectManagementModule, const OBJ_SIZE: usize>
    Benchmark<AllocateMinBenchmarkOptions> for AllocateMinBenchmark<'a, 'b, A, M, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            let heap_inner = self.heap.get_inner().borrow_mut();
            assert!(
                !heap_inner.get_non_resident_allocator().get_free_list()[self.object_bucket_index]
                    .is_empty()
            );
        }

        let timer = T::start();

        let item = black_box(self.heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE])).unwrap();
        let res = timer.stop();

        drop(item);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> AllocateMinBenchmarkOptions {
        AllocateMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>>()
        }
    }
}
