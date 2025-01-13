use std::{alloc::Layout, marker::PhantomData, mem::size_of};

use serde::Serialize;

use crate::{
    benchmarks::{
        locked_wcet::{maximize_hole_list_length, LockedWCETExecutor, ResidentObjectMetadata},
        Timer,
    }, modules::allocator::AllocatorModule, resident_object_manager::resident_object::ResidentObject, util::repr_c_layout
};

#[derive(Serialize)]
pub(crate) struct ResidentObjectManager1LockedWCETExecutorOptions {
    buffer_size: usize,
    object_size: usize,
}

pub(crate) struct ResidentObjectManager1LockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    object_size: usize,
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer>
    ResidentObjectManager1LockedWCETExecutor<'a, TIMER>
{
    pub(crate) fn new(buffer: &'a mut [u8], object_size: usize) -> Self {
        Self {
            buffer,
            object_size,
            _phantom_data: PhantomData,
        }
    }
}

impl<'a, 'b, A: AllocatorModule, TIMER: Timer>
    LockedWCETExecutor<'a, A, ResidentObjectManager1LockedWCETExecutorOptions>
    for ResidentObjectManager1LockedWCETExecutor<'b, TIMER>
{
    fn execute(
        &mut self,
        _storage_ref: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedStorageReference<
            'static,
            'static,
        >,
        heap: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedPersistLock<'a, *mut A>,
        enable_measurement: &std::sync::atomic::AtomicBool,
    ) -> u32 {
        let layouts = [
            Layout::new::<ResidentObjectMetadata>(),
            Layout::from_size_align(self.object_size, 1).unwrap(),
        ];
        let layout = repr_c_layout(&layouts).unwrap();
        assert!(layout.size() > size_of::<usize>());

        // maximize the amount of holes for this allocator
        unsafe {
            let guard = heap.try_lock().unwrap();

            let aref = guard.as_mut().unwrap();
            aref.init(&mut self.buffer[0], self.buffer.len());

            maximize_hole_list_length(aref, layout, Layout::new::<ResidentObject<usize>>());
        }

        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let guard = heap.try_lock_measured::<TIMER>().unwrap();

            let res = guard.as_mut().unwrap().allocate(layout).unwrap(); // O(n)
            let dirty_size = ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false); // O(1)

            if 1 < dirty_size {
                guard.as_mut().unwrap().deallocate(res, layout); // O(n)
            }

            guard.measured_drop()
        }
    }

    fn get_name(&self) -> &'static str {
        "resident_object_manager_1_locked_wcet"
    }

    fn get_bench_options(&self) -> ResidentObjectManager1LockedWCETExecutorOptions {
        ResidentObjectManager1LockedWCETExecutorOptions {
            buffer_size: self.buffer.len(),
            object_size: self.object_size,
        }
    }
}
