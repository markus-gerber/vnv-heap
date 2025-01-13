use std::{alloc::Layout, marker::PhantomData, mem::size_of};

use serde::Serialize;

use crate::{
    benchmarks::{
        locked_wcet::{maximize_hole_list_length, maximize_resident_object_list_length, LockedWCETExecutor, ResidentObjectMetadata},
        Timer,
    }, modules::allocator::AllocatorModule, resident_object_manager::{resident_list::ResidentList, resident_object::ResidentObject}, util::repr_c_layout
};

#[derive(Serialize)]
pub(crate) struct ResidentObjectManager3LockedWCETExecutorOptions {
    buffer_size: usize,
    object_size: usize,
    variant: u8,
}

pub(crate) struct ResidentObjectManager3LockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    object_size: usize,
    variant: bool, // variant == true will maximize the amount of holes, variant == false will maximize the amount of ResidentObjects
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer>
    ResidentObjectManager3LockedWCETExecutor<'a, TIMER>
{
    pub(crate) fn new(variant: bool, buffer: &'a mut [u8], object_size: usize) -> Self {
        Self {
            buffer,
            object_size,
            variant,
            _phantom_data: PhantomData,
        }
    }
}

impl<'a, 'b, A: AllocatorModule, TIMER: Timer>
    LockedWCETExecutor<'a, A, ResidentObjectManager3LockedWCETExecutorOptions>
    for ResidentObjectManager3LockedWCETExecutor<'b, TIMER>
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

        let resident_list = ResidentList::new();

        // maximize the amount of holes for this allocator
        let (meta_ptr, raw_ptr) = unsafe {
            let guard = heap.try_lock().unwrap();

            let aref = guard.as_mut().unwrap();
            aref.init(&mut self.buffer[0], self.buffer.len());

            let objs = if self.variant {
                maximize_hole_list_length(aref, layout, Layout::new::<ResidentObject<usize>>())
            } else {
                maximize_resident_object_list_length(aref, layout, Layout::new::<ResidentObject<usize>>())
            };

            for obj in objs {
                let ptr = obj.as_ptr() as *mut ResidentObjectMetadata;
                ptr.write(ResidentObjectMetadata::new::<usize>(0, false));
                resident_list.insert(ptr.as_mut().unwrap());
            }

            let res = guard.as_mut().unwrap().allocate(layout).unwrap();
            let ptr = res.as_ptr() as *mut ResidentObjectMetadata;
            ptr.write(ResidentObjectMetadata::new::<usize>(0, false));
            resident_list.insert(ptr.as_mut().unwrap());
            (ptr, res)
        };

        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let guard = heap.try_lock_measured::<TIMER>().unwrap();
            let _ = resident_list.remove(meta_ptr);

            guard.as_mut().unwrap().deallocate(raw_ptr, layout); // O(n)

            guard.measured_drop()
        }
    }

    fn get_name(&self) -> &'static str {
        "resident_object_manager_3_locked_wcet"
    }

    fn get_bench_options(&self) -> ResidentObjectManager3LockedWCETExecutorOptions {
        ResidentObjectManager3LockedWCETExecutorOptions {
            buffer_size: self.buffer.len(),
            object_size: self.object_size,
            variant: match self.variant {
                true => 1,
                false => 0
            }
        }
    }
}

