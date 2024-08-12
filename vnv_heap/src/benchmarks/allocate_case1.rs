use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, VNVHeap, VNVObject
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct AllocateCase1BenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateCase1Benchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
    const BLOCKER_SIZE: usize
> {
    
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
    blocker: VNVObject<'a, 'b, [u8; BLOCKER_SIZE], A, NonResidentBuddyAllocatorModule<16>, M>,
    blockers: [usize; 16],
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize
    > AllocateCase1Benchmark<'a, 'b, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>) -> Self {
        let mut blocker = heap
            .allocate::<[u8; BLOCKER_SIZE]>([0u8; BLOCKER_SIZE])
            .unwrap();
        {
            // require blocker to be inside RAM
            blocker.get().unwrap();
        }
        {
            let mut inner = heap.get_inner().borrow_mut();
            assert_eq!(inner.get_resident_object_manager().resident_object_count, 1);
            assert_eq!(inner.get_resident_object_manager().resident_object_meta_backup.len(), 1);
        }

        blocker.unload().unwrap();

        let mut blockers = [0; 16];

        {
            let mut inner = heap.get_inner().borrow_mut();
            assert_eq!(inner.get_resident_object_manager().resident_object_count, 0);
            assert_eq!(inner.get_resident_object_manager().resident_object_meta_backup.len(), 1);

            let (storage, _, allocator) = inner.get_modules_mut();
            let free_list = allocator.get_free_list_mut();
            let mut pop = false;
            for (i, bucket) in free_list.iter_mut().enumerate().rev() {
                if !bucket.is_empty() {
                    if !pop {
                        // this is the biggest item, don't remove it but remove
                        // all items that are next
                        pop = true;
                        // println!("the biggest bucket is {}", 2_i32.pow(i as u32));
                    } else {
                        blockers[i] = bucket.pop(storage).unwrap().unwrap();
                        assert!(bucket.is_empty());
                    }
                }
            }

            drop(inner);
        }

        Self {
            heap,
            blockers,
            blocker
        }
    }
}

impl<
        'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize
    > Benchmark<AllocateCase1BenchmarkOptions> for AllocateCase1Benchmark<'a, '_, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_case_1"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            let mut inner = self.heap.get_inner().borrow_mut();
            let (_, _, allocator) = inner.get_modules_mut();
            let free_list = allocator.get_free_list_mut();
            let mut found = false;
            for (i, bucket) in free_list.iter_mut().enumerate().rev() {
                if !found && !bucket.is_empty() {
                    found = true;
                } else {
                    assert!(bucket.is_empty(), "bucket {} should be empty", i);
                }
            }
        }

        let timer = T::start();

        let item = black_box(self.heap.allocate::<[u8; OBJ_SIZE]>(black_box([0u8; OBJ_SIZE]))).unwrap();
        let res = timer.stop();

        drop(item);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> AllocateCase1BenchmarkOptions {
        AllocateCase1BenchmarkOptions {
            object_size: OBJ_SIZE,
            modules: ModuleOptions::new::<A, NonResidentBuddyAllocatorModule<16>>()
        }
    }
}

impl<
        'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize
    > Drop for AllocateCase1Benchmark<'a, '_, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
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
