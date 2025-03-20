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
use serde::Serialize;

use super::{common::single_page::MemoryManager, AllocatorModule, Benchmark, ModuleOptionsBaseline, Timer};

#[derive(Serialize)]
pub struct BaselineDeallocateMinBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineDeallocateMinBenchmark<
    'a,
    'b: 'a,
    const OBJ_SIZE: usize,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule
> {
    memory_manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule
    > BaselineDeallocateMinBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
        manager.get_inner().require_resident(0).unwrap();

        Self {
            memory_manager: manager
        }
    }
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule
    > Benchmark<BaselineDeallocateMinBenchmarkOptions>
    for BaselineDeallocateMinBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_deallocate_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let obj = self.memory_manager.allocate(0, [0u8; OBJ_SIZE]).unwrap();

        let timer = T::start();
        black_box(drop(obj));
        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineDeallocateMinBenchmarkOptions {
        BaselineDeallocateMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

