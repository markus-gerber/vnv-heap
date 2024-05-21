use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
    VNVHeap,
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, Timer};

#[derive(Serialize)]
pub struct AllocateMaxBenchmarkOptions {
    object_size: usize,
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateMaxBenchmark<
    'a,
    A: AllocatorModule,
    S: PersistentStorageModule,
    F: Fn(&mut [u8], usize) -> VNVHeap<A, NonResidentBuddyAllocatorModule<16>, S>,
    const OBJ_SIZE: usize,
> {
    heap_generator: F,
    resident_buffer: &'a mut [u8],
}

impl<
        'a,
        A: AllocatorModule,
        S: PersistentStorageModule,
        F: Fn(&mut [u8], usize) -> VNVHeap<A, NonResidentBuddyAllocatorModule<16>, S>,
        const OBJ_SIZE: usize,
    > AllocateMaxBenchmark<'a, A, S, F, OBJ_SIZE>
{
    pub fn new(heap_generator: F, resident_buffer: &'a mut [u8]) -> Self {
        Self {
            heap_generator,
            resident_buffer,
        }
    }
}

impl<
        'a,
        A: AllocatorModule,
        S: PersistentStorageModule,
        F: Fn(&mut [u8], usize) -> VNVHeap<A, NonResidentBuddyAllocatorModule<16>, S>,
        const OBJ_SIZE: usize,
    > Benchmark<AllocateMaxBenchmarkOptions> for AllocateMaxBenchmark<'a, A, S, F, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_max"
    }

    #[inline]
    fn prepare_next_iteration(&mut self) {}

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let heap = (self.heap_generator)(self.resident_buffer, self.resident_buffer.len());

        let timer = T::start();

        let item = black_box(heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE])).unwrap();
        let res = timer.stop();

        drop(item);
        drop(heap);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> &AllocateMaxBenchmarkOptions {
        &AllocateMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
        }
    }
}
