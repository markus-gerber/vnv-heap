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

use std::marker::PhantomData;

use serde::Serialize;

use crate::{
    benchmarks::{locked_wcet::LockedWCETExecutor, Timer},
    modules::allocator::AllocatorModule,
};

#[derive(Serialize)]
pub(crate) struct StorageLockedWCETExecutorOptions {
    object_size: usize,
}

pub(crate) struct StorageLockedWCETExecutor<'a, TIMER: Timer> {
    buffer: &'a mut [u8],
    _phantom_data: PhantomData<TIMER>,
}

impl<'a, TIMER: Timer> StorageLockedWCETExecutor<'a, TIMER> {
    pub(crate) fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            buffer,
            _phantom_data: PhantomData,
        }
    }
}

impl<'a, 'b, A: AllocatorModule, TIMER: Timer>
    LockedWCETExecutor<'a, A, StorageLockedWCETExecutorOptions>
    for StorageLockedWCETExecutor<'b, TIMER>
{
    fn execute(
        &mut self,
        storage_ref: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedStorageReference<
            'static,
            'static,
        >,
        _heap: &mut crate::benchmarks::locked_wcet::BenchmarkableSharedPersistLock<'a, *mut A>,
        enable_measurement: &std::sync::atomic::AtomicBool,
    ) -> u32 {
        enable_measurement.store(true, std::sync::atomic::Ordering::SeqCst);

        storage_ref
            .write_benchmarked::<TIMER>(0, self.buffer)
            .unwrap()
    }

    fn get_name(&self) -> &'static str {
        "storage_locked_wcet"
    }

    fn get_bench_options(&self) -> StorageLockedWCETExecutorOptions {
        StorageLockedWCETExecutorOptions {
            object_size: self.buffer.len(),
        }
    }
}
