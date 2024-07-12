use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, resident_object_manager::get_total_resident_size, VNVHeap, VNVObject
};
use core::hint::black_box;
use std::array::from_fn;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};

#[derive(Serialize)]
pub struct GetMax1BenchmarkOptions {
    object_size: usize,
    blocker_size: usize,
    blocker_count: usize,
    modules: ModuleOptions
}

pub struct GetMax1Benchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    const OBJ_SIZE: usize,
    const BLOCKER_SIZE: usize,
    const BLOCKER_COUNT: usize,
> {
    object: VNVObject<'a, 'b, [u8; OBJ_SIZE], A, N, M>,
    blockers: [VNVObject<'a, 'b, [u8; BLOCKER_SIZE], A, N, M>; BLOCKER_COUNT],
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
        const BLOCKER_COUNT: usize,
    > GetMax1Benchmark<'a, 'b, A, N, M, OBJ_SIZE, BLOCKER_SIZE, BLOCKER_COUNT>
{
    pub fn new<S: PersistentStorageModule>(heap: &'a VNVHeap<'b, A, N, M, S>, resident_buffer_size: usize) -> Self {
        assert_eq!(resident_buffer_size, heap.get_inner().borrow_mut().get_resident_object_manager().get_remaining_dirty_size());

        assert!(BLOCKER_COUNT * (BLOCKER_SIZE + 16) >= resident_buffer_size, "Blockers should be able to easily fill resident buffer");

        let item = heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap();
        
        Self {
            object: item,
            blockers: from_fn(|_| {
                heap.allocate::<[u8; BLOCKER_SIZE]>([0u8; BLOCKER_SIZE]).unwrap()
            })
        }
    }
}


pub const fn get_resident_size<T>() -> usize {
    get_total_resident_size::<T>()
}

#[macro_export]
macro_rules! get_max_1_benchmark_calc_blocker_size {
    ($val:expr) => {
        {
            use vnv_heap::benchmarks::get_resident_size;
            const METADATA_SIZE: usize = get_resident_size::<()>();
            const RES_SIZE: usize = $val - METADATA_SIZE;
            RES_SIZE
        }
    };
}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
        const BLOCKER_SIZE: usize,
        const BLOCKER_COUNT: usize,
    > Benchmark<GetMax1BenchmarkOptions>
    for GetMax1Benchmark<'a, 'b, A, N, M, OBJ_SIZE, BLOCKER_SIZE, BLOCKER_COUNT>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_max_1"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        // load all blocker objects into memory and make them dirty
        for blocker in self.blockers.iter_mut() {
            drop(blocker.get_mut().unwrap());
        }

        // resident buffer should be completely filled with dirty objects by now
        // the new object has to sync and unload not needed objects

        // TODO its worse too if some references are in use too

        let timer = T::start();

        let item_ref = black_box(self.object.get().unwrap());
        let res = timer.stop();

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMax1BenchmarkOptions {
        GetMax1BenchmarkOptions {
            object_size: OBJ_SIZE,
            blocker_count: BLOCKER_COUNT,
            blocker_size: BLOCKER_SIZE,
            modules: ModuleOptions::new::<A, N>()
        }
    }
}


#[derive(Serialize)]
pub struct GetMax2BenchmarkOptions {
    object_size: usize,
    blocker_size: usize,
    modules: ModuleOptions
}

pub struct GetMax2Benchmark<
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
    > GetMax2Benchmark<'a, 'b, A, N, M, OBJ_SIZE, BLOCKER_SIZE>
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
    > Benchmark<GetMax2BenchmarkOptions>
    for GetMax2Benchmark<'a, 'b, A, N, M, OBJ_SIZE, BLOCKER_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_max_2"
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

        // TODO its worse too if some references are in use too

        let timer = T::start();

        let item_ref = black_box(self.object.get().unwrap());
        let res = timer.stop();

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMax2BenchmarkOptions {
        GetMax2BenchmarkOptions {
            object_size: OBJ_SIZE,
            blocker_size: BLOCKER_SIZE,
            modules: ModuleOptions::new::<A, N>()
        }
    }
}

