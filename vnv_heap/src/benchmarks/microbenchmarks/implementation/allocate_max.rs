use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, VNVHeap, VNVObject
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct AllocateMaxBenchmarkOptions {
    object_size: usize,
    modules: ModuleOptions
}

/// This benchmark only works with the NonResidentBuddyAllocatorModule
pub struct AllocateMaxBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule + 'static,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
    const OBJ_SIZE: usize,
    const BLOCKER_SIZE: usize
> {
    heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>,
    blockers: [usize; 16],
    blocker: VNVObject<'a, 'b, [u8; BLOCKER_SIZE], A, NonResidentBuddyAllocatorModule<16>, M>,
    debug_obj: VNVObject<'a, 'b, (), A, NonResidentBuddyAllocatorModule<16>, M>,
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize
    > AllocateMaxBenchmark<'a, 'b, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    pub fn new(heap: &'a VNVHeap<'b, A, NonResidentBuddyAllocatorModule<16>, M, S>) -> Self {
        
        let blocker = heap
            .allocate::<[u8; BLOCKER_SIZE]>([0u8; BLOCKER_SIZE])
            .unwrap();

        let debug_obj = heap.allocate::<()>(()).unwrap();

        let mut blockers = [0; 16];

        {
            let mut inner = heap.get_inner().borrow_mut();
            assert_eq!(inner.get_resident_object_manager().resident_object_count, 1);

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
            blocker,
            debug_obj
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
    > Benchmark<AllocateMaxBenchmarkOptions> for AllocateMaxBenchmark<'a, '_, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "allocate_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        // load blocker object into memory and make it dirty
        let blocker_ref = match self.blocker.get() {
            Ok(res) => res,
            Err(_) => {
                println!("Could not get mutable reference for blocker!");
                panic!("Could not get mutable reference for blocker!");
            }
        };
        {
            // it should not be possible to load debug object (size 0) into resident buffer without unloading the blocker object
            assert!(
                self.debug_obj.get().is_err(),
                "Loading debug object should result in an error"
            );
        }
        {
            let mut heap_inner = self.heap.get_inner().borrow_mut();
            let (storage, resident, non_resident) = heap_inner.get_modules_mut();

            assert_eq!(resident.resident_object_count, 1);
        }
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
        drop(blocker_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> AllocateMaxBenchmarkOptions {
        AllocateMaxBenchmarkOptions {
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
    > Drop for AllocateMaxBenchmark<'a, '_, A, M, S, OBJ_SIZE, BLOCKER_SIZE>
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
