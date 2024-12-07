use crate::modules::persistent_storage::PersistentStorageModule;
use core::hint::black_box;
use std::{alloc::Layout, any::TypeId, mem::size_of, ptr::NonNull};
use serde::Serialize;

use super::{common::MemoryManager, AllocatorModule, Benchmark, LinkedListAllocatorModule, ModuleOptionsBaseline, Timer};

// // calculates the amount of 
// pub const fn baseline_allocate_max_min_calc_blocker_obj_cnt<A: AllocatorModule>(obj_size: usize, bucket_size: usize) -> usize {
//     // minimum allocation size
//     let blocker_size = if TypeId::of::<A>() == TypeId::of::<LinkedListAllocatorModule>() {
//         size_of::<usize>() * 2
//     } else {
//         panic!("not implemented")
//     };
    
//     let rem_size = bucket_size - size_of::<A>();
//     (rem_size - obj_size) / blocker_size
// }

// for this we want to fragment our linked-list-allocator until the memory looks like this:
// [FREE - TOO SMALL] [BLOCKER_SIZE] [FREE - TOO SMALL] [BLOCKER_SIZE] ... [BLOCKER_SIZE] [FREE]

// minimum size of an object that can be allocated
// this is only for the linked list allocator
const BLOCKER_SIZE: usize = size_of::<usize>() * 2;

type BLOCKER = [u8; BLOCKER_SIZE];

#[derive(Serialize)]
pub struct BaselineAllocateMaxBenchmarkOptions {
    object_size: usize,
    bucket_size: usize,
    modules: ModuleOptionsBaseline,
}

pub struct BaselineAllocateMaxBenchmark<
    'a,
    'b: 'a,
    const OBJ_SIZE: usize,
    const BUCKET_SIZE: usize,
    A: AllocatorModule + 'static,
    S: PersistentStorageModule
> {
    memory_manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>,
    blocker_objs: Vec<NonNull<u8>>
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule + 'static,
        S: PersistentStorageModule
    > BaselineAllocateMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(memory_manager: &'a mut MemoryManager<'b, BUCKET_SIZE, A, S>) -> Self {
        if TypeId::of::<A>() != TypeId::of::<LinkedListAllocatorModule>() {
            panic!("not implemented. BLOCKER_SIZE only implemented for LinkedListAllocatorModule");
        }

        let mut blocker_objs: Vec<NonNull<u8>> = vec![];
        let mut to_drop_objs: Vec<NonNull<u8>> = vec![];

        memory_manager.get_inner().require_resident(0).unwrap();
        
        loop {
            {
                // fill heap with blockers as long as allocating the actual object succeeds
                let obj = memory_manager.allocate(0, [0u8; OBJ_SIZE]);
                if obj.is_err() {
                    break;
                }

                // drop this object again now
            }
            
            assert_eq!(memory_manager.get_inner().curr_resident_bucket(), 0);
            let blocker = unsafe { memory_manager.get_inner().allocator().allocate(Layout::new::<BLOCKER>()).unwrap() };
            if blocker_objs.len() == to_drop_objs.len() {
                to_drop_objs.push(blocker);
            } else {
                blocker_objs.push(blocker);
            }
        }

        Self::deallocate_blockers(&mut to_drop_objs, memory_manager);

        {
            if blocker_objs.len() == to_drop_objs.len() {
                // the last item was added to blocker_objs
                // however as the loop above was canceled, we know that an object can't fit into the memory anymore
                // so now we have to deallocate the last blocker obj
                
                if let Some(ptr) = blocker_objs.pop() {
                    unsafe { memory_manager.get_inner().allocator().deallocate(ptr, Layout::new::<BLOCKER>()) };
                }
            }

            
            // now, allocation should succeed
            let obj = memory_manager.allocate(0, [0u8; OBJ_SIZE]);
            assert!(obj.is_ok());

            // deallocate obj again
        }

        Self {
            memory_manager,
            blocker_objs
        }
    }


    fn deallocate_blockers<'c>(blockers: &mut Vec<NonNull<u8>>, manager: &'c mut MemoryManager<'b, BUCKET_SIZE, A, S>) {
        for blocker in blockers {
            unsafe { manager.get_inner().allocator().deallocate(*blocker, Layout::new::<BLOCKER>()) };
        }
    }
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule + 'static,
        S: PersistentStorageModule
    > Benchmark<BaselineAllocateMaxBenchmarkOptions>
    for BaselineAllocateMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "baseline_allocate_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        {
            self.memory_manager.get_inner().require_resident(1).unwrap();
            self.memory_manager.get_inner().make_dirty();
            assert_eq!(self.memory_manager.get_inner().curr_resident_bucket(), 1);
        }

        let timer = T::start();
        let obj = black_box(self.memory_manager.allocate(0, [0u8; OBJ_SIZE])).unwrap();
        let res = timer.stop();
        drop(obj);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> BaselineAllocateMaxBenchmarkOptions {
        BaselineAllocateMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            bucket_size: BUCKET_SIZE,
            modules: ModuleOptionsBaseline::new::<A>()
        }
    }
}

impl<
        'a,
        'b: 'a,
        const OBJ_SIZE: usize,
        const BUCKET_SIZE: usize,
        A: AllocatorModule + 'static,
        S: PersistentStorageModule
    > Drop
    for BaselineAllocateMaxBenchmark<'a, 'b, OBJ_SIZE, BUCKET_SIZE, A, S>
{
    fn drop(&mut self) {
        Self::deallocate_blockers(&mut self.blocker_objs, self.memory_manager);
    }
}