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
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct GetMinMaxBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions,
}

pub struct GetMinMaxBenchmark<
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
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule + 'static,
        const OBJ_SIZE: usize,
    > GetMinMaxBenchmark<'a, 'b, A, N, M, S, OBJ_SIZE>
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
        let object = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();

        Self {
            _heap: heap,
            object,
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
    > Benchmark<GetMinMaxBenchmarkOptions> for GetMinMaxBenchmark<'a, 'b, A, N, M, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_min_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        // prepare run
        {
            // unload object
            self.object.unload().unwrap();
            debug_assert!(!self.object.is_resident(), "object should not be resident anymore");
        }

        let timer = T::start();

        let item_ref = black_box(self.object.get().unwrap());
        let res = timer.stop();

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMinMaxBenchmarkOptions {
        GetMinMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, N>(),
        }
    }
}
