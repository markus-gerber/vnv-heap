use crate::modules::persistent_storage::PersistentStorageModule;
use core::hint::black_box;
use serde::Serialize;

use super::{common::single_page::MemoryManager, AllocatorModule, Benchmark, ModuleOptionsBaseline, Timer};

#[derive(Serialize)]
pub struct BaselineAllocateMinMaxBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineAllocateMinMaxBenchmark<
    'a,
    'b: 'a,
    const OBJ_SIZE: usize,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule
> {
    memory_manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule
    > BaselineAllocateMinMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
        manager.get_inner().require_resident(0).unwrap();

        Self {
            memory_manager: manager
        }
    }
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule
    > Benchmark<BaselineAllocateMinMaxBenchmarkOptions>
    for BaselineAllocateMinMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_allocate_min_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            self.memory_manager.get_inner().require_resident(1).unwrap();
            debug_assert!(!self.memory_manager.get_inner().is_dirty());
            debug_assert_eq!(self.memory_manager.get_inner().curr_resident_bucket(), 1);
        }

        let timer = T::start();
        let obj = black_box(self.memory_manager.allocate(0, [0u8; OBJ_SIZE])).unwrap();
        let res = timer.stop();

        drop(obj);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineAllocateMinMaxBenchmarkOptions {
        BaselineAllocateMinMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

