use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, VNVHeap
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct AllocateCase1BenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateCase1Benchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > AllocateCase1Benchmark<'a, 'b, A, M, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>) -> Self {
        
        Self {
            heap,
        }
    }
}

impl<
        'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > Benchmark<AllocateCase1BenchmarkOptions> for AllocateCase1Benchmark<'a, '_, A, M, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_case_1"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let timer = T::start();

        let item = black_box(self.heap.allocate::<[u8; OBJ_SIZE]>(black_box([0u8; OBJ_SIZE]))).unwrap();
        let res = timer.stop();

        drop(item);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> AllocateCase1BenchmarkOptions {
        AllocateCase1BenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>>()
        }
    }
}
