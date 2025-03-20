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

use crate::{
    modules::{
        allocator::AllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    },
    resident_object_manager::{
        get_total_resident_size, resident_object::ResidentObject
    },
    VNVHeap, VNVObject,
};
use core::hint::black_box;
use serde::Serialize;
use std::mem::{needs_drop, size_of};

use super::{Benchmark, ModuleOptions, Timer};

struct DeallocateDropRequiredObject<const SIZE: usize> {
    #[allow(dead_code)]
    inner: [u8; SIZE],
}

impl<const SIZE: usize> Drop for DeallocateDropRequiredObject<SIZE> {
    fn drop(&mut self) {
        black_box(1);
    }
}

#[derive(Serialize)]
pub struct DeallocateMaxBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions,
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct DeallocateMaxBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
    const BLOCKER_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
    blocker: VNVObject<'a, 'b, [u8; BLOCKER_SIZE], A, NonResidentBuddyAllocatorModule<16>, M>,
    debug_obj: VNVObject<'a, 'b, (), A, NonResidentBuddyAllocatorModule<16>, M>,
    blockers: [usize; 16],
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
    > DeallocateMaxBenchmark<'a, 'b, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    pub fn new(
        heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
        resident_buffer_size: usize,
    ) -> Self {
        assert!(
            needs_drop::<DeallocateDropRequiredObject<OBJ_SIZE>>(),
            "Object should need drop for worst case scenario"
        );
        assert!(
            size_of::<ResidentObject<[u8; OBJ_SIZE]>>() <= resident_buffer_size,
            "{} > {}",
            size_of::<ResidentObject<[u8; OBJ_SIZE]>>(),
            resident_buffer_size
        );
        assert_eq!(
            heap.get_inner()
                .borrow_mut()
                .get_resident_object_manager()
                .get_remaining_dirty_size(),
            resident_buffer_size,
            "whole buffer should be able to be dirty"
        );
        // blocker size should been calculated with this function
        assert_eq!(
            resident_buffer_size,
            get_total_resident_size::<[u8; BLOCKER_SIZE]>(),
            "blocker size is wrong! {} != {}",
            resident_buffer_size,
            get_total_resident_size::<[u8; BLOCKER_SIZE]>()
        );

        let mut blocker = heap
            .allocate::<[u8; BLOCKER_SIZE]>([0u8; BLOCKER_SIZE])
            .unwrap();
        blocker.unload().unwrap();


        let mut debug_obj = heap.allocate::<()>(()).unwrap();
        debug_obj.unload().unwrap();

        {
            let mut inner = heap.get_inner().borrow_mut();
            let (storage, obj_manager, allocator) = inner.get_modules_mut();
        }

        let mut blockers = [0; 16];

        {
            let mut inner = heap.get_inner().borrow_mut();

            let (storage, _, allocator) = inner.get_modules_mut();
            let free_list = allocator.get_free_list_mut();
            let mut pop = false;
            for (i, bucket) in free_list.iter_mut().enumerate().rev() {
                if !bucket.is_empty() {
                    if !pop {
                        // this is the biggest item, don't remove it but remove
                        // all items that are next
                        pop = true;
                    } else {
                        blockers[i] = bucket.pop(storage).unwrap().unwrap();
                        assert!(bucket.is_empty());
                    }
                }
            }

            drop(inner);
        }

        Self {
            heap,
            blocker,
            debug_obj,
            blockers,
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
    > Benchmark<DeallocateMaxBenchmarkOptions>
    for DeallocateMaxBenchmark<'a, 'b, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "deallocate_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            let mut inner = self.heap.get_inner().borrow_mut();

            let (_, _, allocator) = inner.get_modules_mut();
            let free_list = allocator.get_free_list_mut();
            let mut found = false;
            for (i, bucket) in free_list.iter_mut().enumerate().rev() {
                if !found && !bucket.is_empty() {
                    found = true;
                } else {
                    assert!(bucket.is_empty(), "bucket {} should be empty", i);
                }
            }
        }

        let obj = self.heap.allocate(DeallocateDropRequiredObject {
            inner: [0u8; OBJ_SIZE],
        });

        {
            // load blocker object into memory and make it dirty
            let blocker_ref = match self.blocker.get_mut() {
                Ok(res) => res,
                Err(_) => {
                    println!("Could not get mutable reference for blocker!");
                    panic!("Could not get mutable reference for blocker!");
                }
            };

            // it should not be possible to load debug object (size 0) into resident buffer without unloading the blocker object
            assert!(
                self.debug_obj.get().is_err(),
                "Loading debug object should result in an error"
            );
            drop(blocker_ref);
        }

        let timer = T::start();

        black_box(drop(black_box(obj)));

        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> DeallocateMaxBenchmarkOptions {
        DeallocateMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>>(),
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
    > Drop for DeallocateMaxBenchmark<'a, 'b, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    fn drop(&mut self) {
        let mut inner = self.heap.get_inner().borrow_mut();

        let (storage, _, allocator) = inner.get_modules_mut();
        let free_list = allocator.get_free_list_mut();
        for (i, ptr) in self.blockers.iter().enumerate() {
            if *ptr != 0 {
                unsafe { free_list[i].push(*ptr as usize, storage).unwrap() };
            }
        }

        drop(inner);
    }
}
