use std::{alloc::Layout, marker::PhantomData, mem::size_of, ptr::NonNull};

use serde::Serialize;

use crate::{
    benchmarks::{
        locked_wcet::{maximize_hole_list_length, LockedWCETExecutor, ResidentObjectMetadata},
        Timer,
    }, modules::allocator::AllocatorModule, resident_object_manager::{resident_list::ResidentList, resident_object::{calc_resident_obj_layout_dynamic, ResidentObject}}, util::repr_c_layout
};

#[derive(Serialize)]
pub(crate) struct ResidentObjectMetadata1LockedWCETExecutorOptions {
    buffer_size: usize,
    object_size: usize,
}

pub(crate) struct ResidentObjectMetadata1LockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    object_size: usize,
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer>
    ResidentObjectMetadata1LockedWCETExecutor<'a, TIMER>
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
    LockedWCETExecutor<'a, A, ResidentObjectMetadata1LockedWCETExecutorOptions>
    for ResidentObjectMetadata1LockedWCETExecutor<'b, TIMER>
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
        let (min_ptr, layout_ptr) = unsafe {
            let guard = heap.try_lock().unwrap();

            let aref = guard.as_mut().unwrap();
            aref.init(&mut self.buffer[0], self.buffer.len());

            let min_ptr = aref.allocate(Layout::new::<ResidentObject<usize>>()).unwrap();

            maximize_hole_list_length(aref, layout, Layout::new::<ResidentObject<usize>>());
            let layout_ptr = guard.as_mut().unwrap().allocate(layout.clone()).unwrap();

            (min_ptr.as_ptr() as *mut ResidentObjectMetadata, layout_ptr.as_ptr() as *mut ResidentObjectMetadata)
        };

        let list = ResidentList::new();

        unsafe {
            min_ptr.write(ResidentObjectMetadata::new::<usize>(0, false));
            layout_ptr.write(ResidentObjectMetadata::new::<usize>(8, false));

            list.insert(min_ptr.as_mut().unwrap());
            list.insert(layout_ptr.as_mut().unwrap());
        }
        let mut iter = list.iter_mut();
        let _ = iter.next().unwrap();
        let delete_handle = iter.next().unwrap();

        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let guard = heap.try_lock_measured::<TIMER>().unwrap();

            // remove from resident object list
            let item_ref = delete_handle.delete(); // O(1)

            let (total_layout, obj_offset) = calc_resident_obj_layout_dynamic( // O(1)
                &item_ref.inner.layout,
                item_ref
                    .inner
                    .status
                    .is_partial_dirtiness_tracking_enabled(),
            );

            // now, as this item is not used anymore, deallocate it
            let resident_obj_ptr = item_ref.to_resident_obj_ptr::<()>() as *mut u8; // O(1)

            let base_ptr = resident_obj_ptr.sub(obj_offset); // O(1)
            let base_ptr = NonNull::new(base_ptr).unwrap(); // O(1)

            guard.as_mut().unwrap().deallocate(base_ptr, total_layout); // O(n)

            guard.measured_drop()
        }
    }

    fn get_name(&self) -> &'static str {
        "resident_object_metadata_1_locked_wcet"
    }

    fn get_bench_options(&self) -> ResidentObjectMetadata1LockedWCETExecutorOptions {
        ResidentObjectMetadata1LockedWCETExecutorOptions {
            buffer_size: self.buffer.len(),
            object_size: self.object_size,
        }
    }
}

