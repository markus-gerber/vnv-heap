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

use std::{marker::PhantomData, sync::atomic::AtomicBool};

use serde::Serialize;

use super::{microbenchmarks::PersistentStorageModule, util::*, AllocatorModule};

use super::Benchmark;

pub(super) trait LockedWCETExecutor<'a, A: AllocatorModule, O: Serialize> {
    fn execute(
        &mut self,
        storage_ref: &mut BenchmarkableSharedStorageReference<'static, 'static>,
        heap: &mut BenchmarkableSharedPersistLock<'a, *mut A>,
        enable_measurement: &AtomicBool
    ) -> u32;

    fn get_name(&self) -> &'static str;

    fn get_bench_options(&self) -> O;
}

pub(super) struct LockedWCETBenchmark<'a, A: AllocatorModule, O: Serialize, E: LockedWCETExecutor<'a, A, O>> {
    storage_ref: BenchmarkableSharedStorageReference<'static, 'static>,
    executor: E,
    heap: BenchmarkableSharedPersistLock<'a, *mut A>,
    _phantom_data: PhantomData<(&'a (), E, O)>,
}

impl<'a, A: AllocatorModule, O: Serialize, E: LockedWCETExecutor<'a, A, O>> LockedWCETBenchmark<'a, A, O, E> {
    pub(super) fn new<S: PersistentStorageModule + 'static>(
        storage: &'a mut S,
        allocator: &'a mut A,
        executor: E,
    ) -> Self {
        let sref: BenchmarkableSharedStorageReference<'_, '_> =
            BenchmarkableSharedStorageReference::new(BenchmarkableSharedPersistLock::new(
                storage,
                &BENCHMARKABLE_PERSIST_QUEUED,
                &BENCHMARKABLE_STORAGE_LOCK,
            ));
        let aref: BenchmarkableSharedPersistLock<'_, *mut A> = BenchmarkableSharedPersistLock::new(
            allocator,
            &BENCHMARKABLE_PERSIST_QUEUED,
            &BENCHMARKABLE_STORAGE_LOCK,
        );

        unsafe {
            BENCHMARKABLE_PERSIST_ACCESS_POINT
                .set(sref.try_lock_clone().unwrap())
                .unwrap()
        };

        Self {
            _phantom_data: PhantomData,
            storage_ref: sref,
            heap: aref,
            executor
        }
    }
}

impl<'a, A: AllocatorModule, O: Serialize, E: LockedWCETExecutor<'a, A, O>> Benchmark<O>
    for LockedWCETBenchmark<'a, A, O, E>
{
    fn get_name(&self) -> &'static str {
        self.executor.get_name()
    }

    fn get_bench_options(&self) -> O {
        self.executor.get_bench_options()
    }

    fn execute<T: crate::benchmarks::Timer>(&mut self) -> u32 {
        self.executor.execute(&mut self.storage_ref, &mut self.heap, &BENCHMARKABLE_PERSIST_QUEUED)
    }
}

impl<'a, A: AllocatorModule, O: Serialize, E: LockedWCETExecutor<'a, A, O>> Drop for LockedWCETBenchmark<'a, A, O, E> {
    fn drop(&mut self) {
        unsafe {
            BENCHMARKABLE_PERSIST_ACCESS_POINT.unset().unwrap();
        }
    }
}
