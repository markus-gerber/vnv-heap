use crate::modules::persistent_storage::PersistentStorageModule;
use core::hint::black_box;
use serde::Serialize;

use super::{common::single_page::MemoryManager, AllocatorModule, Benchmark, ModuleOptionsBaseline, Timer};

#[derive(Serialize)]
pub struct BaselineDeallocateMinMaxBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineDeallocateMinMaxBenchmark<
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
    > BaselineDeallocateMinMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
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
    > Benchmark<BaselineDeallocateMinMaxBenchmarkOptions>
    for BaselineDeallocateMinMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_deallocate_min_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let obj = self.memory_manager.allocate(0, [0u8; OBJ_SIZE]).unwrap();
        self.memory_manager.get_inner().require_resident(1).unwrap();
        debug_assert!(!self.memory_manager.get_inner().is_dirty());
        debug_assert_eq!(self.memory_manager.get_inner().curr_resident_bucket(), 1);

        let timer = T::start();
        black_box(drop(obj));
        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineDeallocateMinMaxBenchmarkOptions {
        BaselineDeallocateMinMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

