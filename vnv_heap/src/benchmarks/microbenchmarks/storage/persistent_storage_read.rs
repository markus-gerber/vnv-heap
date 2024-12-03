use std::{any::type_name, hint::black_box};

use serde::Serialize;

use crate::modules::persistent_storage::PersistentStorageModule;

use super::Benchmark;

#[derive(Serialize)]
pub struct PersistentStorageReadBenchmarkOptions {
    object_size: usize,
    persistent_storage_module: &'static str
}

pub struct PersistentStorageReadBenchmark<'a, S: PersistentStorageModule> {
    storage_module: &'a mut S,
    data: &'a mut [u8]
}
impl<'a, S: PersistentStorageModule> PersistentStorageReadBenchmark<'a, S> {
    pub fn new(data: &'a mut [u8], storage_module: &'a mut S) -> Self {
        Self {
            storage_module,
            data
        }
    }
}

impl<'a, S: PersistentStorageModule> Benchmark<PersistentStorageReadBenchmarkOptions> for PersistentStorageReadBenchmark<'a, S> {
    fn get_name(&self) -> &'static str {
        "persistent_storage_read"
    }

    fn get_bench_options(&self) -> PersistentStorageReadBenchmarkOptions {
        PersistentStorageReadBenchmarkOptions {
            object_size: self.data.len(),
            persistent_storage_module: type_name::<S>()
        }
    }

    fn execute<T: super::Timer>(&mut self) -> u32 {
        let timer = T::start();

        black_box(self.storage_module.read(0, black_box(&mut self.data))).unwrap();

        timer.stop()
    }
}