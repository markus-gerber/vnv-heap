use super::{run_kvs_application_equiv_obj_len, AccessType, KeyValueStoreImpl};
use crate::benchmarks::Benchmark;
use core::marker::PhantomData;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub(super) struct VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions: Serialize + Clone> {
    pub(super) iterations: usize,
    pub(super) object_count: usize,
    pub(super) object_size: usize,
    pub(super) access_type: AccessType,
    pub(super) kvs_options: KVSOptions
}

pub(super) struct KeyValueStoreBenchmark<InternalPointer, KVSOptions: Serialize + Clone, I: KeyValueStoreImpl<InternalPointer>> {
    implementation: I,
    phantom_data: PhantomData<InternalPointer>,
    name: &'static str,
    options: VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions>,
}

impl<InternalPointer, KVSOptions: Serialize + Clone, I: KeyValueStoreImpl<InternalPointer>>
    KeyValueStoreBenchmark<InternalPointer, KVSOptions, I>
{
    pub(super) fn new(
        implementation: I,
        name: &'static str,
        options: VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions>,
    ) -> Self {
        Self {
            phantom_data: PhantomData,
            implementation,
            name,
            options,
        }
    }
}

impl<InternalPointer, KVSOptions: Serialize + Clone, I: KeyValueStoreImpl<InternalPointer>>
    Benchmark<VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions>> for KeyValueStoreBenchmark<InternalPointer, KVSOptions, I>
{
    fn get_name(&self) -> &'static str {
        self.name
    }

    fn get_bench_options(&self) -> VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions> {
        self.options.clone()
    }

    fn execute<T: crate::benchmarks::Timer>(&mut self) -> u32 {
        run_kvs_application_equiv_obj_len::<256, InternalPointer, I, T>(
            &mut self.implementation,
            self.options.object_count,
            self.options.iterations,
            self.options.access_type
        )
    }
}
