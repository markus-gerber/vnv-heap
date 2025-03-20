/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::{alloc::Layout, marker::PhantomData, mem::size_of};

use memoffset::offset_of;
use serde::Serialize;

use crate::{
    benchmarks::{
        locked_wcet::{maximize_hole_list_length, maximize_resident_object_list_length, LockedWCETExecutor, ResidentObjectMetadata},
        Timer,
    }, modules::allocator::AllocatorModule, resident_object_manager::{resident_list::ResidentList, resident_object::{calc_resident_obj_layout_dynamic, ResidentObject}}, util::repr_c_layout
};

#[derive(Serialize)]
pub(crate) struct ResidentObjectManager2LockedWCETExecutorOptions {
    buffer_size: usize,
    object_size: usize,
    variant: u8,
}

pub(crate) struct ResidentObjectManager2LockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    object_size: usize,
    variant: bool, // variant == true will maximize the amount of holes, variant == false will maximize the amount of ResidentObjects
    remaining_dirty_size: usize, // only for simulation purposes
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer>
    ResidentObjectManager2LockedWCETExecutor<'a, TIMER>
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
    LockedWCETExecutor<'a, A, ResidentObjectManager2LockedWCETExecutorOptions>
    for ResidentObjectManager2LockedWCETExecutor<'b, TIMER>
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
        let (_, res_obj_offset) =
            calc_resident_obj_layout_dynamic(&layout, false);

        let dirty_size =
            ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);

        self.remaining_dirty_size = 100;
        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);
        unsafe {
            let guard = heap.try_lock_measured::<TIMER>().unwrap();

            let obj_ptr = guard.as_mut().unwrap().allocate(layout).unwrap();
            self.remaining_dirty_size -= 10;

            // read data now and store it to the allocated region in memory
            let resident_obj_ptr = obj_ptr.as_ptr().add(res_obj_offset);

            let meta_ptr = resident_obj_ptr.add(offset_of!(ResidentObject<usize>, metadata))
                as *mut ResidentObjectMetadata;
            meta_ptr.write(ResidentObjectMetadata::new::<usize>(
                10,
                false,
            ));

            {
                // some checks and append to resident list
                let meta_ref = meta_ptr.as_mut().unwrap();

                // this will not do anything if partial dirtiness tracking is disabled
                meta_ref
                    .inner
                    .partial_dirtiness_tracking_info
                    .get_wrapper(meta_ptr)
                    .reset();

                debug_assert_eq!(
                    dirty_size,
                    meta_ref.dirty_size(),
                    "Dirty size of newly created metadata should match const value"
                );

                // will be inserted at the end of the list
                resident_list.insert(meta_ref); // O(n)
            }


            guard.measured_drop()
        }
    }

    fn get_name(&self) -> &'static str {
        "resident_object_manager_2_locked_wcet"
    }

    fn get_bench_options(&self) -> ResidentObjectManager2LockedWCETExecutorOptions {
        ResidentObjectManager2LockedWCETExecutorOptions {
            buffer_size: self.buffer.len(),
            object_size: self.object_size,
            variant: match self.variant {
                true => 1,
                false => 0
            }
        }
    }
}

