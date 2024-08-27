use crate::{
    modules::{
        allocator::AllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    },
    VNVHeap,
};
use core::hint::black_box;
use serde::Serialize;


use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct DeallocateCase1BenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions,
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct DeallocateCase1Benchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
}

impl<'a, 'b: 'a, A: AllocatorModule, M: ObjectManagementModule, S: PersistentStorageModule, const OBJ_SIZE: usize>
    DeallocateCase1Benchmark<'a, 'b, A, M, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>) -> Self {
        
        Self {
            heap: heap,
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > Benchmark<DeallocateCase1BenchmarkOptions> for DeallocateCase1Benchmark<'a, 'b, A, M, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "deallocate_case_1"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let item = self
            .heap
            .allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE])
            .unwrap();

        {
            drop(item.get().unwrap());
            assert!(item.is_resident(), "item should be resident");
        }

        let timer = T::start();

        black_box(drop(black_box(item)));

        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> DeallocateCase1BenchmarkOptions {
        DeallocateCase1BenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>>(),
        }
    }
}
