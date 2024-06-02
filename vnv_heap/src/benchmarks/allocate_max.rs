use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
    VNVHeap,
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct AllocateMaxBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateMaxBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    S: PersistentStorageModule,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, S>,
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > AllocateMaxBenchmark<'a, 'b, A, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, S>) -> Self {
        Self {
            heap,
        }
    }
}

impl<
        'a,
        A: AllocatorModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > Benchmark<AllocateMaxBenchmarkOptions> for AllocateMaxBenchmark<'a, '_, A, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        // TODO: allocate free buckets except the biggest one

        let timer = T::start();

        let item = black_box(self.heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE])).unwrap();
        let res = timer.stop();

        drop(item);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> AllocateMaxBenchmarkOptions {
        AllocateMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>, S>()
        }
    }
}
