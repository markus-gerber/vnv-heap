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
    benchmarks::applications::queue::run_queue_application, modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, vnv_list::VNVList, VNVHeap
};
use std::usize;
use serde::Serialize;

use super::{super::super::{Benchmark, ModuleOptions, Timer}, Queue};

#[derive(Serialize)]
pub struct QueueVNVHeapBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions,
    queue_length: usize,
    iterations: usize,
    buffer_size: usize,
    ram_overhead: usize
}

pub struct QueueVNVHeapBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    const OBJ_SIZE: usize,
> {
    list: VNVList<'a, 'b, [u8; OBJ_SIZE], A, N, M>,
    queue_length: usize,
    iterations: usize,
    buffer_size: usize,
    ram_overhead: usize
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
    > QueueVNVHeapBenchmark<'a, 'b, A, N, M, OBJ_SIZE>
{
    pub fn new<S: PersistentStorageModule>(
        heap: &'a VNVHeap<'b, A, N, M, S>,
        queue_length: usize,
        iterations: usize,
        buffer_size: usize,
        ram_overhead: usize
    ) -> Self {
        let list = heap.new_list::<[u8; OBJ_SIZE]>();

        Self {
            list,
            queue_length,
            iterations,
            buffer_size,
            ram_overhead
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
    > Benchmark<QueueVNVHeapBenchmarkOptions>
    for QueueVNVHeapBenchmark<'a, 'b, A, N, M, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "queue"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        run_queue_application::<OBJ_SIZE, Self, T>(self, self.queue_length, self.iterations)
    }

    #[inline]
    fn get_bench_options(&self) -> QueueVNVHeapBenchmarkOptions {
        QueueVNVHeapBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, N>(),
            iterations: self.iterations,
            queue_length: self.queue_length,
            buffer_size: self.buffer_size,
            ram_overhead: self.ram_overhead
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
    > Queue<OBJ_SIZE> for QueueVNVHeapBenchmark<'a, 'b, A, N, M, OBJ_SIZE>
{
    fn produce(&mut self, data: [u8; OBJ_SIZE]) {
        self.list.push_front(data).unwrap();
    }

    fn consume(&mut self) -> Option<[u8; OBJ_SIZE]> {
        self.list.pop_back().unwrap()
    }
    
    fn capacity(&self) -> usize {
        usize::MAX
    }
}
