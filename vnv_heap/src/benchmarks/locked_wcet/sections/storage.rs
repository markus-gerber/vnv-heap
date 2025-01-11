use std::marker::PhantomData;

use serde::Serialize;

use crate::{
    benchmarks::{locked_wcet::LockedWCETExecutor, Timer},
    modules::allocator::AllocatorModule,
};

#[derive(Serialize)]
pub(crate) struct StorageLockedWCETExecutorOptions {
    object_size: usize,
}

pub(crate) struct StorageLockedWCETExecutor<'a, const ACCESS_SIZE: usize, TIMER: Timer> {
    buffer: &'a mut [u8; ACCESS_SIZE],
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, const ACCESS_SIZE: usize, TIMER: Timer> StorageLockedWCETExecutor<'a, ACCESS_SIZE, TIMER> {
    pub(crate) fn new(buffer: &'a mut [u8; ACCESS_SIZE]) -> Self {
        Self {
            buffer,
            _phantom_data: PhantomData,
        }
    }
}

impl<'a, 'b, A: AllocatorModule, const ACCESS_SIZE: usize, TIMER: Timer>
    LockedWCETExecutor<'a, A, StorageLockedWCETExecutorOptions>
    for StorageLockedWCETExecutor<'b, ACCESS_SIZE, TIMER>
{
    fn execute(
        &mut self,
        storage_ref: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedStorageReference<
            'static,
            'static,
        >,
        _heap: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedPersistLock<'a, *mut A>,
        enable_measurement: &std::sync::atomic::AtomicBool,
    ) -> u32 {
        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);

        storage_ref
            .write_benchmarked::<TIMER>(0, self.buffer)
            .unwrap()
    }

    fn get_name(&self) -> &'static str {
        "storage_locked_wcet"
    }

    fn get_bench_options(&self) -> StorageLockedWCETExecutorOptions {
        StorageLockedWCETExecutorOptions {
            object_size: self.buffer.len(),
        }
    }
}
