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
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    },
    VNVHeap, VNVObject,
};
use core::hint::black_box;
use std::{mem::size_of, ops::Deref};
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

const SMALLEST_OBJ_SIZE: usize = size_of::<usize>();
type SmallestObjData = [u8; SMALLEST_OBJ_SIZE];

#[derive(Serialize)]
pub struct GetMaxMinBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions,
}

pub struct GetMaxMinBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
> {
    _heap: &'a VNVHeap<'b, A, N, M, S>,
    object: VNVObject<'a, 'b, [u8; OBJ_SIZE], A, N, M>,
    // others are small objects that are resident too
    others: Vec<VNVObject<'a, 'b, SmallestObjData, A, N, M>>,
    other_ptrs: Vec<usize>,
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule + 'static,
        const OBJ_SIZE: usize,
    > GetMaxMinBenchmark<'a, 'b, A, N, M, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, N, M, S>, resident_buffer_size: usize) -> Self {
        assert_eq!(
            heap.get_inner()
                .borrow_mut()
                .get_resident_object_manager()
                .get_remaining_dirty_size(),
            resident_buffer_size,
            "whole buffer should be able to be dirty"
        );
        let mut object = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();
        let mut others: Vec<VNVObject<'a, 'b, SmallestObjData, A, N, M>> = vec![];
        let mut other_ptrs = vec![];

        object.unload().unwrap();
        loop {
            if object.get().is_err() {
                // we could not load our main object into memory anymore
                // we allocated enough other objects
                break;
            }

            object.unload().unwrap();

            let new_obj = heap.allocate([0; SMALLEST_OBJ_SIZE]).unwrap();

            // pin this object in RAM
            let pin_res = unsafe {
                heap.get_inner()
                    .borrow_mut()
                    .get_ref(new_obj.get_alloc_id(), false)
            };
            if let Ok(ptr) = pin_res {
                others.push(new_obj);
                other_ptrs.push(ptr as usize)
            } else {
                // could not make resident
                // deallocate and break
                break;
            }
        }

        // okay now we allocated too many objects, as "object" cannot be resident anymore
        // so unpin and deallocate the last one of the other objects now
        let last = others.pop().unwrap();
        let _ = other_ptrs.pop().unwrap();

        unsafe {
            heap.get_inner()
                .borrow_mut()
                .release_ref(last.get_alloc_id())
        };
        drop(last);

        // we should now be able to load our object in again
        assert!(object.get().is_ok());

        Self {
            _heap: heap,
            object,
            others,
            other_ptrs
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule + 'static,
        const OBJ_SIZE: usize,
    > Benchmark<GetMaxMinBenchmarkOptions> for GetMaxMinBenchmark<'a, 'b, A, N, M, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_max_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {

        {
            debug_assert!(self.object.is_resident(), "object should be resident");
            for other in self.others.iter() {
                assert!(other.is_resident());
            }

            let obj_ptr = self.object.get().unwrap().deref().as_ptr() as usize;
            for other in self.other_ptrs.iter() {
                assert!(*other < obj_ptr);
            }
        }

        let timer = T::start();

        let item_ref = black_box(self.object.get().unwrap());
        let res = timer.stop();

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMaxMinBenchmarkOptions {
        GetMaxMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, N>(),
        }
    }
}

impl<
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule + 'static,
        const OBJ_SIZE: usize,
    > Drop for GetMaxMinBenchmark<'_, '_, A, N, M, S, OBJ_SIZE>
{
    fn drop(&mut self) {
        for obj in &self.others {
            unsafe {
                self._heap
                    .get_inner()
                    .borrow_mut()
                    .release_ref(obj.get_alloc_id())
            };
        }
    }
}
