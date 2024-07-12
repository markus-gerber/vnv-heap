use crate::{
    modules::{
        allocator::AllocatorModule,
        nonresident_allocator::{NonResidentAllocatorModule, NonResidentBuddyAllocatorModule},
        object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    },
    resident_object_manager::{
        resident_object::ResidentObject, resident_object_backup::ResidentObjectMetadataBackup,
        MetadataBackupList,
    },
    VNVHeap,
};
use core::hint::black_box;
use serde::Serialize;
use std::{
    alloc::Layout,
    cmp::max,
    mem::{align_of, size_of},
};

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct DeallocateMinBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions,
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct DeallocateMinBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
    object_bucket_index: usize,
    blockers: [usize; 16],
}

impl<'a, 'b: 'a, A: AllocatorModule, M: ObjectManagementModule, S: PersistentStorageModule, const OBJ_SIZE: usize>
    DeallocateMinBenchmark<'a, 'b, A, M, S, OBJ_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>) -> Self {
        let mut blockers = [0; 16];
        let bucket_size = max(
            size_of::<ResidentObject<[u8; OBJ_SIZE]>>().next_power_of_two(),
            max(
                align_of::<ResidentObject<[u8; OBJ_SIZE]>>(),
                size_of::<usize>(),
            ),
        );
        let bucket_index = bucket_size.trailing_zeros();

        {
            let mut inner = heap.get_inner().borrow_mut();
            let (storage, obj_manager, allocator) = inner.get_modules_mut();

            assert_eq!(obj_manager.resident_object_count, 0);

            if obj_manager.resident_object_meta_backup.is_empty() {
                let ptr = allocator
                    .allocate(MetadataBackupList::item_layout(), storage)
                    .unwrap();
                unsafe {
                    obj_manager.resident_object_meta_backup.push(
                        ptr,
                        ResidentObjectMetadataBackup::new_unused(),
                        storage,
                    )
                }
                .unwrap();
            }

            if allocator.get_free_list()[bucket_index as usize].is_empty() {
                // we need to make sure that there is a pointer in this bucket
                // we can just allocate one object of the same bucket size

                blockers[bucket_index as usize] = allocator
                    .allocate(Layout::new::<[u8; OBJ_SIZE]>(), storage)
                    .unwrap();
            }

            let free_list = allocator.get_free_list_mut();
            for (i, bucket) in free_list.iter_mut().enumerate() {
                if !bucket.is_empty() && i != bucket_index as usize {
                    blockers[i] = bucket.pop(storage).unwrap().unwrap();
                    assert!(bucket.is_empty());
                }
            }

            drop(inner);
        }

        Self {
            heap: heap,
            blockers,
            object_bucket_index: bucket_index as usize,
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > Benchmark<DeallocateMinBenchmarkOptions> for DeallocateMinBenchmark<'a, 'b, A, M, S, OBJ_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "deallocate_min"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        let item = self
            .heap
            .allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE])
            .unwrap();

        {
            let heap_inner = self.heap.get_inner().borrow_mut();
            assert!(
                heap_inner.get_non_resident_allocator().get_free_list()[self.object_bucket_index]
                    .is_empty(),
                "Make sure that if you deallocate, no two buckets can get merged"
            );
        }

        let timer = T::start();

        black_box(drop(item));

        let res = timer.stop();

        res
    }

    #[inline]
    fn get_bench_options(&self) -> DeallocateMinBenchmarkOptions {
        DeallocateMinBenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>>(),
        }
    }
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
    > Drop for DeallocateMinBenchmark<'a, 'b, A, M, S, OBJ_SIZE>
{
    fn drop(&mut self) {
        let mut inner = self.heap.get_inner().borrow_mut();

        let (storage, _, allocator) = inner.get_modules_mut();
        let free_list = allocator.get_free_list_mut();
        for (i, ptr) in self.blockers.iter().enumerate() {
            if *ptr != 0 {
                unsafe { free_list[i].push(*ptr as usize, storage).unwrap() };
            }
        }

        drop(inner);
    }
}
