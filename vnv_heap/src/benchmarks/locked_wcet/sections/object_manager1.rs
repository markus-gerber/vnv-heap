use std::{alloc::Layout, marker::PhantomData};

use serde::Serialize;

use crate::{
    benchmarks::{
        locked_wcet::{LockedWCETExecutor, ResidentObjectMetadata},
        Timer,
    },
    modules::allocator::AllocatorModule,
    util::repr_c_layout,
};

#[derive(Serialize)]
pub(crate) struct ObjectManager1LockedWCETExecutorOptions {
    buffer_size: usize,
    object_size: usize,
}

pub(crate) struct ObjectManager1LockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    object_size: usize,
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer>
    ObjectManager1LockedWCETExecutor<'a, TIMER>
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
    LockedWCETExecutor<'a, A, ObjectManager1LockedWCETExecutorOptions>
    for ObjectManager1LockedWCETExecutor<'b, TIMER>
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

        // maximize the amount of holes for this allocator
        unsafe {
            let guard = heap.try_lock().unwrap();

            let aref = guard.as_mut().unwrap();
            aref.init(&mut self.buffer[0], self.buffer.len());

            let mut prev_ptr;
            let mut dealloc_ptrs = vec![];
            loop {
                if let Ok(res) = aref.allocate(Layout::new::<usize>()) {
                    let res2 = aref.allocate(Layout::new::<usize>());
                    if let Ok(res2) = res2 {
                        dealloc_ptrs.push(res);

                        prev_ptr = Some(res2);
                    } else {
                        aref.deallocate(res, Layout::new::<usize>());

                        // "layout" should still fit??
                        break;
                    }
                } else {
                    // "layout" should still fit??
                    break;
                }

                if let Ok(res) = aref.allocate(layout.clone()) {
                    // still fits, continue
                    aref.deallocate(res, layout.clone());
                } else {
                    aref.deallocate(
                        prev_ptr.expect(
                            "Should have a prev ptr, or the object did not fit in the first place!",
                        ),
                        Layout::new::<usize>(),
                    );
                    break;
                }
            }

            // deallocate all holes
            for ptr in dealloc_ptrs {
                aref.deallocate(ptr, Layout::new::<usize>());
            }

            // verify that our object really fits
            if let Ok(res) = aref.allocate(layout.clone()) {
                // still fits, continue
                aref.deallocate(res, layout.clone());
            } else {
                panic!("should not happen!");
            }
        }

        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);
        {
            let guard = heap.try_lock_measured::<TIMER>().unwrap();

            unsafe {
                if let Ok(ptr) = guard.as_mut().unwrap().allocate(layout.clone()) {
                    guard.as_mut().unwrap().deallocate(ptr, layout.clone());
                }
            }

            guard.measured_drop()
        }
    }

    fn get_name(&self) -> &'static str {
        "object_manager_1_locked_wcet"
    }

    fn get_bench_options(&self) -> ObjectManager1LockedWCETExecutorOptions {
        ObjectManager1LockedWCETExecutorOptions {
            buffer_size: self.buffer.len(),
            object_size: self.object_size,
        }
    }
}
