use std::{
    hint::black_box,
    mem::size_of,
};

use serde::Serialize;

use crate::{
    modules::object_management::DefaultObjectManagementModule, resident_object_manager::{
        resident_object::{calc_resident_obj_layout_static, ResidentObject},
        resident_object_metadata::ResidentObjectMetadata,
    }, util::round_up_to_nearest, VNVHeap, VNVObject
};

use super::{
    LinkedListAllocatorModule, NonResidentBuddyAllocatorModule, PersistBenchmark,
    PersistentStorageModule,
};

const fn rem_space(_dirty_size: usize, buf_size: usize, obj_cnt: usize, rem_size: usize) -> usize {
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
const fn does_fit(_dirty_size: usize, buf_size: usize, obj_cnt: usize, rem_size: usize) -> bool {
    let obj_size = size_of::<ResidentObject<usize>>() * obj_cnt;

    let rem_size_total = if rem_size == 0 {
        0
    } else {
        if rem_size % size_of::<usize>() != 0 {
            panic!("x");
        }
        size_of::<ResidentObjectMetadata>() + rem_size
    };

    assert!(buf_size - (obj_size + rem_size_total) >= 2 * size_of::<usize>() || buf_size - (obj_size + rem_size_total) == 0);
    buf_size >= obj_size + rem_size_total
}

const fn remaining_dirty_size(dirty_size: usize, _buf_size: usize, obj_cnt: usize, dirty_obj_cnt: usize, rem_size: usize, rem_dirty: bool) -> usize {
    let mut res = ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false) * obj_cnt;
    res += size_of::<usize>() * dirty_obj_cnt;

    if rem_size != 0 {
        res += ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);
        if rem_dirty {
            res += rem_size;
        }
    }

    if dirty_size >= res {
        dirty_size - res
    } else {
        panic!("err");
    }
}


pub(super) const fn calc_obj_cnt_and_rem_size_max_dirty(
    dirty_size: usize,
    buf_size: usize,
) -> (usize, usize, usize, bool) {
    
    const fn final_check(dirty_size: usize, buf_size: usize, obj_cnt: usize, dirty_obj_cnt: usize, rem_size: usize, rem_dirty: bool) {
        assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));
        let rem_dirty = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        let metadata_dirty_size = ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);
        let metadata_size = size_of::<ResidentObjectMetadata>();

        let rem_space = rem_space(dirty_size, buf_size, obj_cnt, rem_size);
        if rem_size == 0 {
            if rem_space >= metadata_size + size_of::<usize>() {
                assert!(rem_dirty < metadata_dirty_size + size_of::<usize>());
            }
        } else {
            if rem_space >= rem_dirty {
                if rem_space - rem_dirty == 0 || rem_space - rem_dirty >= 2*size_of::<usize>() {
                    assert!(rem_dirty < size_of::<usize>());
                }
            } else if rem_dirty > size_of::<usize>() {
                let add_dirty_size = obj_cnt * (metadata_size - metadata_dirty_size);
                assert!(add_dirty_size < size_of::<usize>());
            }
        }
    }

    let obj_size = size_of::<ResidentObject<usize>>();
    let metadata_dirty_size = ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);
    let metadata_size = size_of::<ResidentObjectMetadata>();
    let max_obj_cnt = buf_size / obj_size;

    let mut obj_cnt = 0;
    let mut dirty_obj_cnt = 0;
    let mut rem_size = 0;
    let mut rem_dirty = false;

    // try to create as much objects as possible
    while obj_cnt < max_obj_cnt {
        let rem_dirty_size =
            remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

        if rem_dirty_size >= metadata_dirty_size {
            obj_cnt += 1;
        } else if rem_dirty_size >= size_of::<usize>() && obj_cnt > 0 {
            obj_cnt -= 1;
            let rem_dirty_size =
                remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

            assert!(rem_dirty_size >= metadata_dirty_size);

            rem_size =
                ((rem_dirty_size - metadata_dirty_size) / size_of::<usize>()) * size_of::<usize>();
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, true);
            return (obj_cnt, dirty_obj_cnt, rem_size, true);
        } else {
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
            return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        }
    }

    let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

    if rem_dirty_size < size_of::<usize>() {
        final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    }

    while dirty_obj_cnt < max_obj_cnt {
        let rem_dirty_size =
            remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

        if rem_dirty_size >= size_of::<usize>() {
            dirty_obj_cnt += 1;
        } else {
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
            return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        }
    }

    let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    if rem_dirty_size < size_of::<usize>() {
        final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    }

    assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));

    // merge objects into one bigger rem obj
    obj_cnt -= 1;
    dirty_obj_cnt -= 1;

    rem_size = 0;
    rem_dirty = false;
    let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    if rem_dirty_size - metadata_dirty_size <= buf_size - obj_cnt * obj_size - metadata_size {
        rem_size = ((rem_dirty_size - metadata_dirty_size) / size_of::<usize>()) * size_of::<usize>();
        rem_dirty = true;

        let rem_space = rem_space(dirty_size, buf_size, obj_cnt, rem_size);
        if rem_space < 2 * size_of::<usize>() && rem_space != 0 {
            rem_size -= round_up_to_nearest(2 * size_of::<usize>() - rem_space, size_of::<usize>());
        }

        assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));

        final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    } else {
        rem_size = ((buf_size - obj_cnt * obj_size - metadata_size) / size_of::<usize>()) * size_of::<usize>();
        rem_dirty = true;

        assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));
    }

    while obj_cnt > 0 {
        let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

        if rem_dirty_size >= size_of::<usize>() {
            obj_cnt -= 1;
            dirty_obj_cnt -= 1;

            rem_size = 0;
            rem_dirty = false;
            let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

            if rem_dirty_size - metadata_dirty_size <= buf_size - obj_cnt * obj_size - metadata_size {
                rem_size = ((rem_dirty_size - metadata_dirty_size) / size_of::<usize>()) * size_of::<usize>();
                rem_dirty = true;
        
                let rem_space = rem_space(dirty_size, buf_size, obj_cnt, rem_size);
                if rem_space < 2 * size_of::<usize>() && rem_space != 0 {
                    rem_size -= round_up_to_nearest(2 * size_of::<usize>() - rem_space, size_of::<usize>());
                }
        
                assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));
        
                final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
                return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
            } else {
                rem_size = ((buf_size - obj_cnt * obj_size - metadata_size) / size_of::<usize>()) * size_of::<usize>();
                rem_dirty = true;
        
                assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));
            }
        } else {
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
            return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        }
    }


    rem_size = 0;
    rem_dirty = false;
    let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

    if rem_dirty_size - metadata_dirty_size <= buf_size - obj_cnt * obj_size - metadata_size {
        rem_size = ((rem_dirty_size - metadata_dirty_size) / size_of::<usize>()) * size_of::<usize>();
        rem_dirty = true;

        let rem_space = rem_space(dirty_size, buf_size, obj_cnt, rem_size);
        if rem_space < 2 * size_of::<usize>() && rem_space != 0 {
            rem_size -= round_up_to_nearest(2 * size_of::<usize>() - rem_space, size_of::<usize>());
        }

        assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));

        final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    } else {
        rem_size = ((buf_size - obj_cnt * obj_size - metadata_size) / size_of::<usize>()) * size_of::<usize>();
        rem_dirty = true;

        assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));
    }

    final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty)
}


pub(super) const fn calc_obj_cnt_and_rem_size_max_latency(
    dirty_size: usize,
    buf_size: usize,
) -> (usize, usize, usize, bool) {
    const fn final_check(dirty_size: usize, buf_size: usize, obj_cnt: usize, dirty_obj_cnt: usize, rem_size: usize, rem_dirty: bool) {
        assert!(does_fit(dirty_size, buf_size, obj_cnt, rem_size));
        let rem_dirty = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        let metadata_dirty_size = ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);
        let metadata_size = size_of::<ResidentObjectMetadata>();

        let rem_space = rem_space(dirty_size, buf_size, obj_cnt, rem_size);
        if rem_size == 0 {
            if rem_space >= metadata_size + size_of::<usize>() {
                assert!(rem_dirty < metadata_dirty_size + size_of::<usize>());
            }
        } else {
            if rem_space >= rem_dirty {
                if rem_space - rem_dirty == 0 || rem_space - rem_dirty >= 2*size_of::<usize>() {
                    assert!(rem_dirty < size_of::<usize>());
                }
            }
        }
    }

    let obj_size = size_of::<ResidentObject<usize>>();
    let metadata_dirty_size = ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false);
    let max_obj_cnt = buf_size / obj_size;

    let mut obj_cnt = 0;
    let mut dirty_obj_cnt = 0;
    let mut rem_size = 0;
    let rem_dirty = false;

    // try to create as much objects as possible
    while obj_cnt < max_obj_cnt {
        let rem_dirty_size =
            remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

        if rem_dirty_size >= metadata_dirty_size {
            obj_cnt += 1;
        } else if rem_dirty_size >= size_of::<usize>() && obj_cnt > 0 {
            obj_cnt -= 1;
            let rem_dirty_size =
                remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

            assert!(rem_dirty_size >= metadata_dirty_size);

            rem_size =
                ((rem_dirty_size - metadata_dirty_size) / size_of::<usize>()) * size_of::<usize>();
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, true);
            return (obj_cnt, dirty_obj_cnt, rem_size, true);
        } else {
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
            return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        }
    }

    let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

    if rem_dirty_size < size_of::<usize>() {
        final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    }

    while dirty_obj_cnt < max_obj_cnt {
        let rem_dirty_size =
            remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

        if rem_dirty_size >= size_of::<usize>() {
            dirty_obj_cnt += 1;
        } else {
            final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
            return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        }
    }

    let rem_dirty_size = remaining_dirty_size(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    if rem_dirty_size < size_of::<usize>() {
        final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
        return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    }

    final_check(dirty_size, buf_size, obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);
    return (obj_cnt, dirty_obj_cnt, rem_size, rem_dirty);

}

#[derive(Serialize)]
pub struct WorstCasePersistLatencyBenchmarkOptions {
    dirty_size: usize,
    buffer_size: usize,
    rem_object_size: usize,
    object_cnt: usize,
    remaining_dirty_size: usize
}

pub(super) struct WorstCasePersistLatencyBenchmark<
    'a,
    'b,
    const BUFFER_SIZE: usize,
    const CUTOFF_SIZE: usize,
    const REM_OBJECT_SIZE: usize,
> {
    dirty_size: usize,
    objects: Vec<
        VNVObject<
            'a,
            'b,
            usize,
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<19>,
            DefaultObjectManagementModule,
        >,
    >,
    _rem_object: Option<
        VNVObject<
            'a,
            'b,
            [u8; REM_OBJECT_SIZE],
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<19>,
            DefaultObjectManagementModule,
        >,
    >,
    benchmark_name: &'static str,
    remaining_dirty_size: usize
}

impl<'a, 'b, const BUFFER_SIZE: usize, const CUTOFF_SIZE: usize, const REM_OBJECT_SIZE: usize>
    WorstCasePersistLatencyBenchmark<'a, 'b, BUFFER_SIZE, CUTOFF_SIZE, REM_OBJECT_SIZE>
{
    pub(super) fn new<S: PersistentStorageModule>(
        dirty_size: usize,
        heap: &'a mut VNVHeap<
            'b,
            LinkedListAllocatorModule,
            NonResidentBuddyAllocatorModule<19>,
            DefaultObjectManagementModule,
            S,
        >,
        normal_objects: usize,
        dirty_normal_objects: usize,
        is_rem_dirty: bool,
        benchmark_name: &'static str
    ) -> Self {
        assert!(dirty_normal_objects <= normal_objects);

        let rem_object = if REM_OBJECT_SIZE != 0 {
            assert!(
                REM_OBJECT_SIZE >= size_of::<usize>(),
                "{} >= {}",
                REM_OBJECT_SIZE,
                size_of::<usize>()
            );

            let mut obj = heap.allocate([0u8; REM_OBJECT_SIZE]).unwrap();

            if is_rem_dirty {
                obj.get_mut().unwrap();
            } else {
                obj.unload().unwrap();
                obj.get().unwrap();
                assert!(!obj.is_data_dirty());
            }

            Some(obj)
        } else {
            None
        };

        let mut objects = vec![];
        let mut dirty_normal_objects_curr = dirty_normal_objects;
        for i in 0..normal_objects {
            let mut obj = heap.allocate(i).unwrap();

            if dirty_normal_objects_curr > 0 {
                obj.get_mut().unwrap();
                dirty_normal_objects_curr -= 1;
            } else {
                obj.unload().unwrap();
                obj.get().unwrap();
                assert!(!obj.is_data_dirty());
            }

            objects.push(obj);
        }


        let mut dirty_normal_objects_curr = dirty_normal_objects;
        for obj in objects.iter_mut() {
            assert!(obj.is_resident(), "metadata dirty size: {}, resident obj size: {}", ResidentObjectMetadata::fresh_object_dirty_size::<usize>(false), calc_resident_obj_layout_static::<usize>(false).0.size());
            if dirty_normal_objects_curr > 0 {
                assert!(obj.is_data_dirty());
                dirty_normal_objects_curr -= 1;    
            } else {
                assert!(!obj.is_data_dirty());    
            }
        }

        if let Some(rem_obj) = rem_object.as_ref() {
            assert!(rem_obj.is_resident(), "{}", calc_resident_obj_layout_static::<usize>(false).0.size());
            assert_eq!(rem_obj.is_data_dirty(), is_rem_dirty);
        }

        let rem_dirty = heap
            .get_inner()
            .borrow_mut()
            .get_resident_object_manager()
            .remaining_dirty_size;

        Self {
            dirty_size,
            objects,
            _rem_object: rem_object,
            benchmark_name,
            remaining_dirty_size: rem_dirty
        }
    }
}

impl<'a, 'b, const BUFFER_SIZE: usize, const CUTOFF_SIZE: usize, const REM_OBJECT_SIZE: usize>
    PersistBenchmark<WorstCasePersistLatencyBenchmarkOptions>
    for WorstCasePersistLatencyBenchmark<'a, 'b, BUFFER_SIZE, CUTOFF_SIZE, REM_OBJECT_SIZE>
{
    fn get_name(&self) -> &'static str {
        self.benchmark_name
    }

    fn get_bench_options(&self) -> WorstCasePersistLatencyBenchmarkOptions {
        WorstCasePersistLatencyBenchmarkOptions {
            buffer_size: BUFFER_SIZE,
            dirty_size: self.dirty_size,
            object_cnt: self.objects.len(),
            rem_object_size: REM_OBJECT_SIZE,
            remaining_dirty_size: self.remaining_dirty_size
        }
    }

    fn loop_until_finished(&mut self, finished: &std::sync::atomic::AtomicBool) {
        while !finished.load(std::sync::atomic::Ordering::SeqCst) {
            // some sanity checks
            for obj in self.objects.iter_mut() {
                assert!(obj.is_resident());
            }

            // some computations
            let mut data: usize = 0;
            for obj in self.objects.iter_mut() {
                data = data.wrapping_add(*obj.get().unwrap());
            }
            black_box(data);
        }
    }
}
