use std::{marker::PhantomData, sync::atomic::AtomicBool};

use serde::Serialize;

use super::{microbenchmarks::PersistentStorageModule, util::*, AllocatorModule};
use crate::benchmarks::GetCurrentTicks;

use super::Benchmark;

pub(super) trait LockedWCETExecutor<'a, A: AllocatorModule> {
    fn execute(
        &mut self,
        storage_ref: &mut BenchmarkableSharedStorageReference<'static, 'static>,
        heap: &mut BenchmarkableSharedPersistLock<'a, *mut A>,
        enable_measurement: &AtomicBool
    ) -> u32;

    fn get_name(&self) -> &'static str;
}

#[derive(Serialize)]
pub(super) struct LockedWCETBenchmarkOptions {}

pub(super) struct LockedWCETBenchmark<'a, A: AllocatorModule, E: LockedWCETExecutor<'a, A>> {
    storage_ref: BenchmarkableSharedStorageReference<'static, 'static>,
    executor: E,
    heap: BenchmarkableSharedPersistLock<'a, *mut A>,
    _phantom_data: PhantomData<(&'a (), E)>,
}

impl<'a, A: AllocatorModule, E: LockedWCETExecutor<'a, A>> LockedWCETBenchmark<'a, A, E> {
    pub(super) fn new<S: PersistentStorageModule + 'static>(
        storage: &'a mut S,
        allocator: &'a mut A,
        executor: E,
    ) -> Self {
        let sref: BenchmarkableSharedStorageReference<'_, '_> =
            BenchmarkableSharedStorageReference::new(BenchmarkableSharedPersistLock::new(
                storage,
                &BENCHMARKABLE_PERSIST_QUEUED,
                &BENCHMARKABLE_STORAGE_LOCK,
            ));
        let aref: BenchmarkableSharedPersistLock<'_, *mut A> = BenchmarkableSharedPersistLock::new(
            allocator,
            &BENCHMARKABLE_PERSIST_QUEUED,
            &BENCHMARKABLE_STORAGE_LOCK,
        );

        unsafe {
            BENCHMARKABLE_PERSIST_ACCESS_POINT
                .set(sref.try_lock_clone().unwrap())
                .unwrap()
        };

        Self {
            _phantom_data: PhantomData,
            storage_ref: sref,
            heap: aref,
            executor
        }
    }
}

impl<'a, A: AllocatorModule, E: LockedWCETExecutor<'a, A>> Benchmark<LockedWCETBenchmarkOptions>
    for LockedWCETBenchmark<'a, A, E>
{
    fn get_name(&self) -> &'static str {
        self.executor.get_name()
    }

    fn get_bench_options(&self) -> LockedWCETBenchmarkOptions {
        LockedWCETBenchmarkOptions {}
    }

    fn execute<T: crate::benchmarks::Timer>(&mut self) -> u32 {
        self.executor.execute(&mut self.storage_ref, &mut self.heap, &BENCHMARKABLE_PERSIST_QUEUED)
    }
}

impl<'a, A: AllocatorModule, E: LockedWCETExecutor<'a, A>> Drop for LockedWCETBenchmark<'a, A, E> {
    fn drop(&mut self) {
        unsafe {
            BENCHMARKABLE_PERSIST_ACCESS_POINT.unset().unwrap();
        }
    }
}
