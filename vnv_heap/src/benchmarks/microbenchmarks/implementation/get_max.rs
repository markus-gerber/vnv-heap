use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule, object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    }, resident_object_manager::{resident_object::ResidentObject, resident_object_metadata::ResidentObjectMetadata}, VNVHeap, VNVObject
};
use core::hint::black_box;
use std::mem::size_of;
use serde::Serialize;

use super::{Benchmark, ModuleOptions, Timer};


const fn rem_space(buf_size: usize, obj_cnt: usize, rem_size: usize) -> usize {
    let obj_size = size_of::<ResidentObject<usize>>() * obj_cnt;

    let rem_size_total = if rem_size == 0 {
        0
    } else {
        if rem_size % size_of::<usize>() != 0 {
            panic!("x");
        }
        size_of::<ResidentObjectMetadata>() + rem_size
    };

    assert!(buf_size >= obj_size + rem_size_total);
    buf_size - (obj_size + rem_size_total)
}
const fn does_fit(buf_size: usize, obj_cnt: usize, rem_size: usize) -> bool {
    let rem = rem_space(buf_size, obj_cnt, rem_size);
    
    assert!(rem >= 2 * size_of::<usize>() || rem == 0);
    // rem_space does already check
    true
}

pub(super) const fn calc_obj_cnt_and_rem_size_get_max(
    benchmark_obj_size: usize,
    buf_size: usize,
) -> (usize, usize) {
    
    const fn final_check(buf_size: usize, benchmark_obj_size: usize, obj_cnt: usize, rem_size: usize) {
        assert!(does_fit(buf_size, obj_cnt, rem_size));
        let metadata_size = size_of::<ResidentObjectMetadata>();
        let obj_size = size_of::<ResidentObject<usize>>();

        let rem_space = rem_space(buf_size, obj_cnt, rem_size);
        if rem_size == 0 {
            assert!(rem_space < metadata_size + size_of::<usize>());
        } else {
            if rem_space == 0 || rem_space >= 2*size_of::<usize>() {
                assert!(rem_space < size_of::<usize>());
            } else {
                assert!(rem_space < size_of::<usize>());
            }
        }

        assert!(benchmark_obj_size > obj_cnt * obj_size);
    }

    let obj_size = size_of::<ResidentObject<usize>>();
    let metadata_size = size_of::<ResidentObjectMetadata>();
    let mut obj_cnt = benchmark_obj_size / obj_size;

    if benchmark_obj_size % obj_size == 0 {
        obj_cnt -= 1;
    }

    let rem_space = rem_space(buf_size, obj_cnt,0);
    let rem_size = ((rem_space - metadata_size) / size_of::<usize>()) * size_of::<usize>();

    final_check(buf_size, benchmark_obj_size, obj_cnt, rem_size);
    (obj_cnt, rem_size)
}


#[derive(Serialize)]
pub struct GetMaxBenchmarkOptions {
    object_size: usize,
    rem_size: usize,
    blocker_cnt: usize,
    modules: ModuleOptions
}

pub struct GetMaxBenchmark<
    'a,
    'b: 'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    const OBJ_SIZE: usize,
    const REM_SIZE: usize
> {
    object: VNVObject<'a, 'b, [u8; OBJ_SIZE], A, N, M>,
    blockers: Vec<VNVObject<'a, 'b, usize, A, N, M>>,
    rem_obj: Option<VNVObject<'a, 'b, [u8; REM_SIZE], A, N, M>>,
    
    debug_obj: VNVObject<'a, 'b, (), A, N, M>

}

impl<
        'a,
        'b: 'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        const OBJ_SIZE: usize,
        const REM_SIZE: usize
    > GetMaxBenchmark<'a, 'b, A, N, M, OBJ_SIZE, REM_SIZE>
{

    pub fn new<S: PersistentStorageModule>(heap: &'a VNVHeap<'b, A, N, M, S>, resident_buffer_size: usize, blocker_cnt: usize) -> Self {
        assert_eq!(heap.get_inner().borrow_mut().get_resident_object_manager().get_remaining_dirty_size(), resident_buffer_size, "whole buffer should be able to be dirty");

        assert_eq!(REM_SIZE % size_of::<usize>(), 0);
        assert_eq!(OBJ_SIZE % size_of::<usize>(), 0);

        let rem_obj = if REM_SIZE != 0 {
            Some(heap.allocate([0; REM_SIZE]).unwrap())
        } else {
            None
        };

        let mut blockers = vec![];
        for _ in 0..blocker_cnt {
            blockers.push(heap.allocate(0).unwrap());
        }

        Self {
            object: heap.allocate::<[u8; OBJ_SIZE]>([0u8; OBJ_SIZE]).unwrap(),
            blockers,
            rem_obj,
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
        const REM_SIZE: usize
    > Benchmark<GetMaxBenchmarkOptions>
    for GetMaxBenchmark<'a, 'b, A, N, M, OBJ_SIZE, REM_SIZE>
{
    #[inline]
    fn get_name(&self) -> &'static str {
        "get_max"
    }

    #[inline]
    fn execute<T: Timer>(&mut self) -> u32 {
        // prepare run
        {
            // load blocker objects into memory and make them dirty
            let rem_mut = if let Some(obj) = self.rem_obj.as_mut() {
                Some(obj.get_mut().unwrap())
            } else {
                None
            };
            
            let mut refs = vec![];
            
            for blocker in self.blockers.iter_mut() {
                refs.push(blocker.get_mut().unwrap());
            };

            // it should not be possible to load debug object (size 0) into resident buffer without unloading the blocker object
            assert!(self.debug_obj.get().is_err(), "Loading debug object should result in an error");
            
            drop(refs);
            drop(rem_mut);


            if let Some(first) = self.blockers.first() {
                let heap = first.get_heap();

                let res_list = heap.get_resident_object_manager().get_resident_list();
                let iter = res_list.iter();
                let mut i = 0;
                for x in iter {
                    if i < self.blockers.len() {
                        assert_eq!(x.inner.layout.size(), size_of::<usize>())
                    } else {
                        assert_eq!(x.inner.layout.size(), REM_SIZE);
                    }
    
                    i += 1;
                }

                drop(heap);
            }
        }

        // resident buffer should be completely filled with dirty objects by now
        // the new object has to sync and unload not needed objects

        let timer = T::start();

        let item_ref = black_box(self.object.get_mut().unwrap());
        let res = timer.stop();

        for blocker in self.blockers.iter() {
            assert!(!blocker.is_resident());
        }

        if let Some(rem) = self.rem_obj.as_ref() {
            assert!(!rem.is_resident());
        }

        drop(item_ref);

        res
    }

    #[inline]
    fn get_bench_options(&self) -> GetMaxBenchmarkOptions {
        GetMaxBenchmarkOptions {
            object_size: OBJ_SIZE,
            rem_size: REM_SIZE,
            blocker_cnt: self.blockers.len(),
            modules: ModuleOptions::new::<A, N>()
        }
    }
}

