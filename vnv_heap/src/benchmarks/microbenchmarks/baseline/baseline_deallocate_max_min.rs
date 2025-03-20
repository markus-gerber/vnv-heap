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

use crate::modules::persistent_storage::PersistentStorageModule;
use core::hint::black_box;
use std::{alloc::Layout, any::TypeId, mem::size_of, ptr::NonNull};
use serde::Serialize;

use super::{common::single_page::MemoryManager, AllocatorModule, Benchmark, LinkedListAllocatorModule, ModuleOptionsBaseline, Timer};

// // calculates the amount of 
// pub const fn baseline_allocate_max_min_calc_blocker_obj_cnt<A: AllocatorModule>(obj_size: usize, bucket_size: usize) -> usize {
//     // minimum allocation size
//     let blocker_size = if TypeId::of::<A>() == TypeId::of::<LinkedListAllocatorModule>() {
//         size_of::<usize>() * 2
//     } else {
//         panic!("not implemented")
//     };
    
//     let rem_size = bucket_size - size_of::<A>();
//     (rem_size - obj_size) / blocker_size
// }

// for this we want to fragment our linked-list-allocator until the memory looks like this:
// [FREE - TOO SMALL] [BLOCKER_SIZE] [FREE - TOO SMALL] [BLOCKER_SIZE] ... [BLOCKER_SIZE] [FREE]

// minimum size of an object that can be allocated
// this is only for the linked list allocator
const BLOCKER_SIZE: usize = size_of::<usize>() * 2;

type BLOCKER = [u8; BLOCKER_SIZE];

#[derive(Serialize)]
pub struct BaselineDeallocateMaxMinBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineDeallocateMaxMinBenchmark<
    'a,
    'b: 'a,
    const OBJ_SIZE: usize,
    const BUCKET_SIZE: usize,
    A: AllocatorModule + 'static,
    S: PersistentStorageModule
> {
    memory_manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>,
    blocker_objs: Vec<NonNull<u8>>
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule + 'static,
        S: PersistentStorageModule
    > BaselineDeallocateMaxMinBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(memory_manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
        if TypeId::of::<A>() != TypeId::of::<LinkedListAllocatorModule>() {
            panic!("not implemented. BLOCKER_SIZE only implemented for LinkedListAllocatorModule");
        }

        let mut blocker_objs: Vec<NonNull<u8>> = vec![];
        let mut to_drop_objs: Vec<NonNull<u8>> = vec![];

        memory_manager.get_inner().require_resident(0).unwrap();
        
        loop {
            {
                // fill heap with blockers as long as allocating the actual object succeeds
                if !Self::check_state(memory_manager) {
                    break;
                }
            }

            assert_eq!(memory_manager.get_inner().curr_resident_bucket(), 0);
            let blocker = unsafe { memory_manager.get_inner().allocator().allocate(Layout::new::<BLOCKER>()).unwrap() };
            if blocker_objs.len() == to_drop_objs.len() {
                to_drop_objs.push(blocker);
            } else {
                blocker_objs.push(blocker);
            }
        }

        {
            // in every case: remove last blocker
            // case 1: blocker was the last object to be allocated, this means:
            //   As the loop above was canceled, we know that an object can't fit into the memory anymore
            //   so now we have to deallocate the last blocker obj
            // case 2: to_drop_obj was the last object to be allocated, this means:
            //   the last real blocker is the object before the object to be allocated
            //   however, we want to be sure that there is a free hole that has to be merged once we deallocate the obj

            if let Some(ptr) = blocker_objs.pop() {
                unsafe { memory_manager.get_inner().allocator().deallocate(ptr, Layout::new::<BLOCKER>()) };
            }
        }
        Self::deallocate_blockers(&mut to_drop_objs, memory_manager);
        assert!(Self::check_state(memory_manager));

        Self {
            memory_manager,
            blocker_objs
        }
    }

    fn check_state<'c>(memory_manager: &'c mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> bool {
        let obj = memory_manager.allocate(0, [0u8; OBJ_SIZE]);
        if obj.is_err() {
            return false;
        }

        if Self::additional_blocker_possible() {
            let rem_space = memory_manager.allocate(0, [0u8; BLOCKER_SIZE]);
            if rem_space.is_err() {
                return false;
            }    
        }

        // drop the objects again

        return true;
    }

    // does an additional blocker even fit for our OBJ_SIZE?
    const fn additional_blocker_possible() -> bool {
        let rem_space = BUCKET_SIZE - size_of::<A>() - OBJ_SIZE;
        return rem_space >= BLOCKER_SIZE;
    }

    fn deallocate_blockers<'c>(blockers: &mut Vec<NonNull<u8>>, manager: &'c mut MemoryManager<'b, BUCKET_SIZE, A, S>) {
        for blocker in blockers {
            unsafe { manager.get_inner().allocator().deallocate(*blocker, Layout::new::<BLOCKER>()) };
        }
    }
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule + 'static,
        S: PersistentStorageModule
    > Benchmark<BaselineDeallocateMaxMinBenchmarkOptions>
    for BaselineDeallocateMaxMinBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_deallocate_max_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        self.memory_manager.get_inner().require_resident(0).unwrap();
        // we need to first occupy one blocker here
        // so our memory looks like this: [FREE: TOO SMALL] [BLOCKER] ... [REM_SPACE_BLOCKER] [OBJ] [FREE]
        // after dropping rem_space it looks like this: [FREE: TOO SMALL] [BLOCKER] ... [FREE] [OBJ] [FREE]

        let rem_space = if Self::additional_blocker_possible() {
            Some(self.memory_manager.allocate(0, [0u8; BLOCKER_SIZE]).unwrap())
        } else {
            // cannot fit another blocker, because obj only will fit without one
            None
        };

        let obj = self.memory_manager.allocate(0, [0u8; OBJ_SIZE]).unwrap();

        assert_eq!(self.memory_manager.get_inner().curr_resident_bucket(), 0);

        drop(rem_space);

        let timer = T::start();
        black_box(drop(obj));
        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineDeallocateMaxMinBenchmarkOptions {
        BaselineDeallocateMaxMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule + 'static,
        S: PersistentStorageModule
    > Drop
    for BaselineDeallocateMaxMinBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    fn drop(&mut self) {
        Self::deallocate_blockers(&mut self.blocker_objs, self.memory_manager);
    }
}