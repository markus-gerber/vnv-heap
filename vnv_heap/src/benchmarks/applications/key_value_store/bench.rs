use super::{run_kvs_application_equiv_obj_len, KeyValueStoreImpl};
use crate::benchmarks::Benchmark;
use core::marker::PhantomData;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub(super) struct VNVHeapKeyValueStoreBenchmarkOptions {
    pub(super) iterations: usize,
    pub(super) obj_cnt: usize,
}

pub(super) struct KeyValueStoreBenchmark<InternalPointer, I: KeyValueStoreImpl<InternalPointer>> {
    implementation: I,
    phantom_data: PhantomData<InternalPointer>,
    name: &'static str,
    options: VNVHeapKeyValueStoreBenchmarkOptions,
}

impl<InternalPointer, I: KeyValueStoreImpl<InternalPointer>>
    KeyValueStoreBenchmark<InternalPointer, I>
{
    pub(super) fn new(
        implementation: I,
        name: &'static str,
        options: VNVHeapKeyValueStoreBenchmarkOptions,
    ) -> Self {
        Self {
            phantom_data: PhantomData,
            implementation,
            name,
            options,
        }
    }
}

impl<InternalPointer, I: KeyValueStoreImpl<InternalPointer>>
    Benchmark<VNVHeapKeyValueStoreBenchmarkOptions> for KeyValueStoreBenchmark<InternalPointer, I>
{
    fn get_name(&self) -> &'static str {
        self.name
    }

    fn get_bench_options(&self) -> VNVHeapKeyValueStoreBenchmarkOptions {
        self.options.clone()
    }

    fn execute<T: crate::benchmarks::Timer>(&mut self) -> u32 {
        run_kvs_application_equiv_obj_len::<256, InternalPointer, I, T>(
            &mut self.implementation,
            self.options.obj_cnt,
            self.options.iterations,
        )
    }
}
