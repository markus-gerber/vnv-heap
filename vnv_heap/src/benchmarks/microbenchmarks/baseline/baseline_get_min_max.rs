use crate::modules::persistent_storage::PersistentStorageModule;
use core::hint::black_box;
use serde::Serialize;

use super::{model::{MemoryManager, Object}, AllocatorModule, Benchmark, ModuleOptionsBaseline, Timer};

#[derive(Serialize)]
pub struct BaselineGetMinMaxBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineGetMinMaxBenchmark<
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
    > BaselineGetMinMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
        assert!(manager.bucket_count() >= 2);
        let obj = manager.allocate(0, [0u8; OBJ_SIZE]).unwrap();
        let blocker = manager.allocate(1, 0u8).unwrap();
        blocker.get_inner().sync().unwrap();

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
    > Benchmark<BaselineGetMinMaxBenchmarkOptions>
    for BaselineGetMinMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_get_min_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            self.blocker.get_ref().unwrap();
            assert!(!self.blocker.get_inner().is_dirty())
        }

        let timer = T::start();

        {
            black_box(self.obj.get_ref()).unwrap();
        }

        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineGetMinMaxBenchmarkOptions {
        BaselineGetMinMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

