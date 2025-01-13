use std::{alloc::Layout, hint::black_box, marker::PhantomData, mem::size_of};

use serde::Serialize;

use crate::{
    benchmarks::{
        locked_wcet::{maximize_hole_list_length, maximize_resident_object_list_length, LockedWCETExecutor, ResidentObjectMetadata},
        Timer,
    }, modules::allocator::AllocatorModule, resident_object_manager::{resident_list::ResidentList, resident_object::ResidentObject}, util::repr_c_layout
};

#[derive(Serialize)]
pub(crate) struct ResidentObjectManager4LockedWCETExecutorOptions {
    buffer_size: usize,
    object_size: usize,
    variant: u8,
}

pub(crate) struct ResidentObjectManager4LockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    object_size: usize,
    variant: bool, // variant == true will maximize the amount of holes, variant == false will maximize the amount of ResidentObjects
    remaining_dirty_size: usize, // only for simulation purposes
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer>
    ResidentObjectManager4LockedWCETExecutor<'a, TIMER>
{
    pub(crate) fn new(variant: bool, buffer: &'a mut [u8], object_size: usize) -> Self {
        Self {
            buffer,
            object_size,
            variant,
            remaining_dirty_size: 0,
            _phantom_data: PhantomData,
        }
    }
}

impl<'a, 'b, A: AllocatorModule, TIMER: Timer>
    LockedWCETExecutor<'a, A, ResidentObjectManager4LockedWCETExecutorOptions>
    for ResidentObjectManager4LockedWCETExecutor<'b, TIMER>
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
        unsafe {
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
        }

        let dirty_size =
            ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);
        self.remaining_dirty_size = 100;

        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);
        let resident_metadata_rel_offset = black_box(0);
        {
            let guard = heap.try_lock_measured::<TIMER>().unwrap();
            
            let res_ptr = unsafe { guard.as_mut().unwrap().allocate(layout) }; // O(n)
            let res_ptr = res_ptr.unwrap();
            let res_ptr = unsafe { res_ptr.as_ptr().add(resident_metadata_rel_offset) };
    
            self.remaining_dirty_size -= dirty_size;
    
            // read data now and store it to the allocated region in memory
            let ptr = res_ptr as *mut ResidentObjectMetadata;
    
            let mut metadata: ResidentObjectMetadata = ResidentObjectMetadata::new::<usize>(
                10,
                false,
            );
            metadata.inner.status.set_data_dirty(true);
            unsafe { ptr.write(metadata) };
    
            {
                // some checks and append to resident list
                let obj_ref = unsafe { ptr.as_mut().unwrap() };
    
                // this does not do anything if partial dirtiness tracking is disabled
                unsafe {
                    obj_ref
                        .inner
                        .partial_dirtiness_tracking_info
                        .get_wrapper(ptr as *mut ResidentObjectMetadata)
                        .reset_and_set_all_blocks_dirty()
                };
    
                unsafe { resident_list.insert(obj_ref) }; // O(n)
            }
    
            guard.measured_drop()
        }
    }

    fn get_name(&self) -> &'static str {
        "resident_object_manager_4_locked_wcet"
    }

    fn get_bench_options(&self) -> ResidentObjectManager4LockedWCETExecutorOptions {
        ResidentObjectManager4LockedWCETExecutorOptions {
            buffer_size: self.buffer.len(),
            object_size: self.object_size,
            variant: match self.variant {
                true => 1,
                false => 0
            }
        }
    }
}

