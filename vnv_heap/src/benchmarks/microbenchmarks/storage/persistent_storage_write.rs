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

use std::{any::type_name, hint::black_box};

use serde::Serialize;

use crate::modules::persistent_storage::PersistentStorageModule;

use super::Benchmark;

#[derive(Serialize)]
pub struct PersistentStorageWriteBenchmarkOptions {
    object_size: usize,
    persistent_storage_module: &'static str
}

pub struct PersistentStorageWriteBenchmark<'a, S: PersistentStorageModule,> {
    storage_module: &'a mut S,
    data: &'a mut [u8]
}

impl<'a, S: PersistentStorageModule> PersistentStorageWriteBenchmark<'a, S> {
    pub fn new(data: &'a mut [u8], storage_module: &'a mut S) -> Self {
        Self {
            storage_module,
            data
        }
    }
}

impl<'a, S: PersistentStorageModule> Benchmark<PersistentStorageWriteBenchmarkOptions> for PersistentStorageWriteBenchmark<'a, S> {
    fn get_name(&self) -> &'static str {
        "persistent_storage_write"
    }

    fn get_bench_options(&self) -> PersistentStorageWriteBenchmarkOptions {
        PersistentStorageWriteBenchmarkOptions {
            object_size: self.data.len(),
            persistent_storage_module: type_name::<S>()
        }
    }

    fn execute<T: super::Timer>(&mut self) -> u32 {
        let timer = T::start();

        black_box(self.storage_module.write(0, black_box(&self.data))).unwrap();

        timer.stop()
    }
}