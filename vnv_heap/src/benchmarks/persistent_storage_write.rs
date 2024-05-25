use std::{any::type_name, hint::black_box};

use serde::Serialize;

use crate::modules::persistent_storage::PersistentStorageModule;

use super::Benchmark;

#[derive(Serialize)]
pub struct PersistentStorageWriteBenchmarkOptions {
    object_size: usize,
    persistent_storage_module: &'static str
}

pub struct PersistentStorageWriteBenchmark<'a, S: PersistentStorageModule, const OBJ_SIZE: usize,> {
    storage_module: &'a mut S
}

impl<'a, S: PersistentStorageModule, const OBJ_SIZE: usize> PersistentStorageWriteBenchmark<'a, S, OBJ_SIZE> {
    pub fn new(storage_module: &'a mut S) -> Self {
        Self {
            storage_module
        }
    }
}

impl<'a, S: PersistentStorageModule, const OBJ_SIZE: usize> Benchmark<PersistentStorageWriteBenchmarkOptions> for PersistentStorageWriteBenchmark<'a, S, OBJ_SIZE> {
    fn get_name(&self) -> &'static str {
        "persistent_storage_write"
    }

    fn get_bench_options(&self) -> PersistentStorageWriteBenchmarkOptions {
        PersistentStorageWriteBenchmarkOptions {
            object_size: OBJ_SIZE,
            persistent_storage_module: type_name::<S>()
        }
    }

    fn execute<T: super::Timer>(&mut self) -> u32 {
        let data = [0u8; OBJ_SIZE];
        
        let timer = T::start();

        black_box(self.storage_module.write(0, &data)).unwrap();

        timer.stop()
    }
}