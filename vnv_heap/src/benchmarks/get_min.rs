use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule
    },
    VNVHeap, VNVObject,
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct GetMinBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

pub struct GetMinBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    const OBJ_SIZE: usize,
> {

    object: VNVObject<'a, 'b, [u8; OBJ_SIZE], A, N, M>,
}

impl<'a, 'b: 'a, A: AllocatorModule + 'static, N: NonResidentAllocatorModule, M: ObjectManagementModule, const OBJ_SIZE: usize>
    GetMinBenchmark<'a, 'b, A, N, M, OBJ_SIZE>
{
    pub fn new<S: PersistentStorageModule>(heap: &'a VNVHeap<'b, A, N, M, S>) -> Self {
        let item = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();
        drop(item.get().unwrap());

        Self {
            object: item,
        }
    }
}

impl<'a, 'b: 'a, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule, const OBJ_SIZE: usize>
    Benchmark<GetMinBenchmarkOptions> for GetMinBenchmark<'a, 'b, A, N, M, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let timer = T::start();

        let item_ref = black_box(self.object.get().unwrap());
        let res = timer.stop();

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMinBenchmarkOptions {
        GetMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, N>()
        }
    }
}
