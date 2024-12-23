use std::mem::{align_of, size_of};

use bench::{calc_obj_cnt_and_rem_size_max_dirty, calc_obj_cnt_and_rem_size_max_objects, PersistLatencyBenchmark};

use super::*;
use crate::{
    benchmarks::{
        BenchmarkRunOptions, BenchmarkRunner, RunAllBenchmarkOptions, Timer,
    }, calc_resident_buf_cutoff_size, modules::allocator::LinkedListAllocatorModule, resident_object_manager::resident_object_metadata::ResidentObjectMetadata, util::round_up_to_nearest
};

use super::{GetCurrentTicks, PersistTrigger, PersistentStorageModule};

const RESIDENT_CUTOFF_SIZE: usize = {
    let tmp = if size_of::<usize>() == 8 {
        // desktop with File Storage Module
        96 + size_of::<usize>()
    } else if size_of::<usize>() == 4 {
        // zephyr with SPI Fram Storage module
        52 + size_of::<usize>()
    } else {
        panic!("uhhm");
    };

    round_up_to_nearest(tmp, align_of::<usize>())
};

// NOTE: if you change one of these three variables
// you also have to update the value in the for_obj_size macro!
const MAX_DIRTY_SIZE: usize = 2 * 1024;
const STEP_SIZE: usize = 32;
const MIN_BUFFER_SIZE: usize = 256;

const MAX_BUFFER_SIZE: usize = 4 * 1024;

const STEP_COUNT: usize = (MAX_BUFFER_SIZE - MIN_BUFFER_SIZE) / STEP_SIZE + 1;

macro_rules! for_buffer_size_impl {
    ($index: ident, $inner: expr, $value: expr) => {
        static_assertions::const_assert_eq!($value, STEP_COUNT);
        static_assertions::const_assert_eq!(MIN_BUFFER_SIZE % STEP_SIZE, 0);
        static_assertions::const_assert_eq!(MAX_BUFFER_SIZE % STEP_SIZE, 0);
        seq_macro::seq!(I in 0..$value {
            {
                const $index: usize = I * STEP_SIZE + MIN_BUFFER_SIZE;
                $inner
            }
        });
    };
}

macro_rules! for_buffer_size {
    ($index: ident, $inner: expr) => {
        for_buffer_size_impl!($index, $inner, 121);
    };
}

pub(crate) struct BufferSizePersistLatencyRunner;

impl BenchmarkRunner for BufferSizePersistLatencyRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_buffer_size_persist_latency {
            iteration_count += 2* STEP_COUNT;
        }

        iteration_count
    }
    fn run<
        TIMER: Timer,
        TRIGGER: PersistTrigger,
        S: PersistentStorageModule + 'static,
        F: Fn() -> S,
        G: FnMut(),
    >(
        run_options: &mut BenchmarkRunOptions,
        options: &RunAllBenchmarkOptions,
        get_storage: &F,
        handle_curr_iteration: &mut G,
        get_ticks: GetCurrentTicks,
    ) {
        type A = LinkedListAllocatorModule;

        assert_eq!(
            RESIDENT_CUTOFF_SIZE,
            calc_resident_buf_cutoff_size::<A, S>(),
            "cutoff size has to match"
        );

        #[allow(unused)]
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
    
        if options.run_buffer_size_persist_latency {
            let mut trigger = TRIGGER::new(trigger_persist);

            for_buffer_size!(BUF_SIZE, {
                handle_curr_iteration();

                const DIRTY_SIZE: usize = {
                    if MAX_DIRTY_SIZE > BUF_SIZE {
                        BUF_SIZE
                    } else {
                        MAX_DIRTY_SIZE
                    }
                };

                const REM_DIRTY_SIZE: usize = DIRTY_SIZE - RESIDENT_CUTOFF_SIZE;
                let mut buf = [0u8; BUF_SIZE];
                let mut heap = get_persist_bench_heap(&mut buf, DIRTY_SIZE, get_storage());
                {
                    let rem_dirty = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;
                    assert_eq!(rem_dirty, REM_DIRTY_SIZE);
                }

                const RES: (usize, usize, usize, bool) = calc_obj_cnt_and_rem_size_max_objects(REM_DIRTY_SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE);
                const OBJ_CNT: usize = RES.0;
                const DIRTY_NORMAL_OBJECTS: usize = RES.1;
                const REM_OBJ_SIZE: usize = RES.2;
                const REM_OBJ_DIRTY: bool = RES.3;

            //    const REST: usize = remaining_dirty_size(REM_DIRTY_SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY);
            //    println!("obj_cnt: {}, dirty_objects: {}, rest: {}, rest dirty?: {}, DIRTYREST: {}", OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY, REST);

                let bench: PersistLatencyBenchmark<
                    BUF_SIZE,
                    RESIDENT_CUTOFF_SIZE,
                    REM_OBJ_SIZE,
                > = PersistLatencyBenchmark::new::<S>(DIRTY_SIZE, &mut heap, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_DIRTY, "max_objects_persist_latency_buffer_size");
                bench.run_benchmark::<TIMER, TRIGGER>(run_options, get_ticks, &mut trigger);
            });

            for_buffer_size!(BUF_SIZE, {
                handle_curr_iteration();

                const DIRTY_SIZE: usize = {
                    if MAX_DIRTY_SIZE > BUF_SIZE {
                        BUF_SIZE
                    } else {
                        MAX_DIRTY_SIZE
                    }
                };
                const REM_DIRTY_SIZE: usize = DIRTY_SIZE - RESIDENT_CUTOFF_SIZE;
                let mut buf = [0u8; BUF_SIZE];
                let mut heap = get_persist_bench_heap(&mut buf, DIRTY_SIZE, get_storage());
                {
                    let rem_dirty = heap.get_inner().borrow_mut().get_resident_object_manager().remaining_dirty_size;
                    assert_eq!(rem_dirty, REM_DIRTY_SIZE);
                }

                const RES: (usize, usize, usize, bool) = calc_obj_cnt_and_rem_size_max_dirty(REM_DIRTY_SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE);
                const OBJ_CNT: usize = RES.0;
                const DIRTY_NORMAL_OBJECTS: usize = RES.1;
                const REM_OBJ_SIZE: usize = RES.2;
                const REM_OBJ_DIRTY: bool = RES.3;
                // const REST: usize = remaining_dirty_size(REM_DIRTY_SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY);
                // println!("obj_cnt: {}, dirty_objects: {}, rest: {}, rest dirty?: {}, DIRTYREST: {}", OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY, REST);


                let bench: PersistLatencyBenchmark<
                    BUF_SIZE,
                    RESIDENT_CUTOFF_SIZE,
                    REM_OBJ_SIZE,
                > = PersistLatencyBenchmark::new::<S>(DIRTY_SIZE, &mut heap, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_DIRTY, "max_dirty_persist_latency_buffer_size");
                bench.run_benchmark::<TIMER, TRIGGER>(run_options, get_ticks, &mut trigger);
            });
        }
    }
}
