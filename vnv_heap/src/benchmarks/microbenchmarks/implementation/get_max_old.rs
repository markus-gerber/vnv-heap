use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, resident_object_manager::get_total_resident_size, VNVHeap, VNVObject
};
use core::hint::black_box;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

pub const fn get_resident_size<T>() -> usize {
    get_total_resident_size::<T>()
}

#[derive(Serialize)]
pub struct GetMaxOldBenchmarkOptions {
    object_size: usize,
    blocker_size: usize,
    modules: ModuleOptions
}

pub struct GetMaxOldBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    const OBJ_SIZE: usize,
    const BLOCKER_SIZE: usize,
> {
    object: VNVObject<'a, 'b, [u8; OBJ_SIZE], A, N, M>,
    blocker: VNVObject<'a, 'b, [u8; BLOCKER_SIZE], A, N, M>,
    debug_obj: VNVObject<'a, 'b, (), A, N, M>
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
    > GetMaxOldBenchmark<'a, 'b, A, N, M, OBJ_SIZE, BLOCKER_SIZE>
{

    pub fn new<S: PersistentStorageModule>(heap: &'a VNVHeap<'b, A, N, M, S>, resident_buffer_size: usize) -> Self {
        assert_eq!(heap.get_inner().borrow_mut().get_resident_object_manager().get_remaining_dirty_size(), resident_buffer_size, "whole buffer should be able to be dirty");
        // blocker size should been calculated with this function
        assert_eq!(resident_buffer_size, get_total_resident_size::<[u8; BLOCKER_SIZE]>(), "blocker size is wrong! {} != {}", resident_buffer_size, get_total_resident_size::<[u8; BLOCKER_SIZE]>());

        Self {
            object: heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap(),
            blocker: heap.allocate::<[u8; BLOCKER_SIZE]>([0u8; BLOCKER_SIZE]).unwrap(),
            debug_obj: heap.allocate::<()>(()).unwrap(),
        }
    }
}


impl<
        'a,
        'b: 'a,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
    > Benchmark<GetMaxOldBenchmarkOptions>
    for GetMaxOldBenchmark<'a, 'b, A, N, M, OBJ_SIZE, BLOCKER_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_max_old"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        // prepare run
        {
            // load blocker object into memory and make them dirty
            let blocker_ref = match self.blocker.get_mut() {
                Ok(res) => res,
                Err(_) => {
                    println!("Could not get mutable reference for blocker!");
                    panic!("Could not get mutable reference for blocker!");
                }
            };

            // it should not be possible to load debug object (size 0) into resident buffer without unloading the blocker object
            assert!(self.debug_obj.get().is_err(), "Loading debug object should result in an error");
            drop(blocker_ref)
        }
        
        // resident buffer should be completely filled with dirty objects by now
        // the new object has to sync and unload not needed objects

        let timer = T::start();

        let item_ref = black_box(self.object.get_mut().unwrap());
        let res = timer.stop();

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMaxOldBenchmarkOptions {
        GetMaxOldBenchmarkOptions {
            object_size: OBJ_SIZE,
            blocker_size: BLOCKER_SIZE,
            modules: ModuleOptions::new::<A, N>()
        }
    }
}

