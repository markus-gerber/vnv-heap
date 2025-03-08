use super::{
    run_kvs_application_bench, AccessType,
    KeyValueStoreImpl, KVS_APP_DIVERSE_OBJ_LEN_OBJ_COUNT_DISTRIBUTION,
    KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES, KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES,
};
use crate::benchmarks::Benchmark;
use core::marker::PhantomData;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub(super) struct VNVHeapKeyValueStoreBenchmarkGeneralOptions<KVSOptions: Serialize + Clone> {
    pub(super) iterations: usize,
    pub(super) object_count: usize,
    pub(super) access_type: AccessType,
    pub(super) kvs_options: KVSOptions,
}

#[derive(Serialize, Clone)]
pub(super) struct VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions: Serialize + Clone> {
    pub(super) inner: VNVHeapKeyValueStoreBenchmarkGeneralOptions<KVSOptions>,
    object_sizes: [usize; KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES],
    object_count_distribution: [usize; KVS_APP_DIVERSE_OBJ_LEN_OBJ_VALUES],
}

pub(super) struct KeyValueStoreBenchmark<
    InternalPointer,
    KVSOptions: Serialize + Clone,
    I: KeyValueStoreImpl<InternalPointer>,
> {
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
        options: VNVHeapKeyValueStoreBenchmarkGeneralOptions<KVSOptions>,
    ) -> Self {
        Self {
            phantom_data: PhantomData,
            implementation,
            name,
            options: VNVHeapKeyValueStoreBenchmarkOptions {
                inner: options,
                object_sizes: KVS_APP_DIVERSE_OBJ_LEN_OBJ_SIZES,
                object_count_distribution: KVS_APP_DIVERSE_OBJ_LEN_OBJ_COUNT_DISTRIBUTION,
            },
        }
    }
}

impl<InternalPointer, KVSOptions: Serialize + Clone, I: KeyValueStoreImpl<InternalPointer>>
    Benchmark<VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions>>
    for KeyValueStoreBenchmark<InternalPointer, KVSOptions, I>
{
    fn get_name(&self) -> &'static str {
        self.name
    }

    fn get_bench_options(&self) -> VNVHeapKeyValueStoreBenchmarkOptions<KVSOptions> {
        self.options.clone()
    }

    fn execute<T: crate::benchmarks::Timer>(&mut self) -> u32 {
        run_kvs_application_bench::<InternalPointer, I, T>(
            &mut self.implementation,
            self.options.inner.object_count,
            self.options.inner.iterations,
            self.options.inner.access_type.clone(),
        )
    }
}
