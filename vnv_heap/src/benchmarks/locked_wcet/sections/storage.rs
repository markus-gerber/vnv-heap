use crate::{benchmarks::locked_wcet::LockedWCETExecutor, modules::allocator::AllocatorModule};

pub(crate) struct StorageLockedWCETExecutor<'a, const ACCESS_SIZE: usize> {
    buffer: &'a mut [u8; ACCESS_SIZE],
}

impl<'a, const ACCESS_SIZE: usize> StorageLockedWCETExecutor<'a, ACCESS_SIZE> {
    pub(crate) fn new(buffer: &'a mut [u8; ACCESS_SIZE]) -> Self {
        Self { buffer }
    }
}

impl<'a, 'b, A: AllocatorModule, const ACCESS_SIZE: usize> LockedWCETExecutor<'a, A>
    for StorageLockedWCETExecutor<'b, ACCESS_SIZE>
{
    fn execute(
        &mut self,
        storage_ref: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedStorageReference<
            'static,
            'static,
        >,
        heap: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedPersistLock<'a, *mut A>,
        enable_measurement: &std::sync::atomic::AtomicBool,
    ) -> u32 {
        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);

        storage_ref.write_benchmarked(0, self.buffer).unwrap()
    }

    fn get_name(&self) -> &'static str {
        "storage_locked_wcet"
    }
}
