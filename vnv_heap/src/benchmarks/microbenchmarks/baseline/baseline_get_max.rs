use crate::modules::persistent_storage::PersistentStorageModule;
use core::hint::black_box;
use serde::Serialize;

use super::{common::single_page::{MemoryManager, Object}, AllocatorModule, Benchmark, ModuleOptionsBaseline, Timer};

#[derive(Serialize)]
pub struct BaselineGetMaxBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineGetMaxBenchmark<
    'a,
    'b: 'a,
    const OBJ_SIZE: usize,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule
> {
    obj: Object<'a, 'b, [u8; OBJ_SIZE], BUCKET_SIZE, A, S>,
    blocker: Object<'a, 'b, u8, BUCKET_SIZE, A, S>
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule
    > BaselineGetMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
        assert!(manager.bucket_count() >= 2);
        let obj = manager.allocate(0, [0u8; OBJ_SIZE]).unwrap();
        let blocker = manager.allocate(1, 0u8).unwrap();

        Self {
            obj,
            blocker
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
    > Benchmark<BaselineGetMaxBenchmarkOptions>
    for BaselineGetMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_get_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            self.blocker.get_mut().unwrap();
            assert!(self.blocker.get_inner().is_dirty())
        }

        let timer = T::start();

        {
            black_box(self.obj.get_ref()).unwrap();
        }

        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineGetMaxBenchmarkOptions {
        BaselineGetMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

