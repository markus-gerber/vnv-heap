/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::mem::{align_of, size_of};

use bench::{calc_obj_cnt_and_rem_size_max_dirty, calc_obj_cnt_and_rem_size_max_objects, PersistLatencyBenchmark};

use super::*;
use crate::{
    benchmarks::{
        BenchmarkRunOptions, BenchmarkRunner, RunAllBenchmarkOptions, Timer,
    }, calc_resident_buf_cutoff_size, modules::{
        persistent_storage::DummyStorageModule,
        allocator::LinkedListAllocatorModule
    }, resident_object_manager::resident_object_metadata::ResidentObjectMetadata, util::round_up_to_nearest, VNVObject
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

const VNV_HEAP_RAM_OVERHEAD: usize = {
    size_of::<VNVHeap<'_, A, N, M, DummyStorageModule>>()
        + size_of::<VNVObject<'_, '_, (), A, N, M>>()
        + VNVHeap::<'_, A, N, M, DummyStorageModule>::get_layout_info().persist_access_point_size
};

// NOTE: if you change one of these three variables
// you also have to update the value in the for_obj_size macro!
const BUF_SIZE: usize = 4 * 1024 - VNV_HEAP_RAM_OVERHEAD;
const STEP_SIZE: usize = 32;
const MAX_DIRTY_SIZE: usize = BUF_SIZE;

const MIN_DIRTY_SIZE: usize = RESIDENT_CUTOFF_SIZE;
const MIN_DIRTY_SIZE_ROUNDED: usize = {
    MAX_DIRTY_SIZE - (((MAX_DIRTY_SIZE - MIN_DIRTY_SIZE) / STEP_SIZE) * STEP_SIZE) 
};

const STEP_COUNT: usize = (MAX_DIRTY_SIZE - MIN_DIRTY_SIZE_ROUNDED) / STEP_SIZE + 1;

macro_rules! for_dirty_size_impl {
    ($index: ident, $inner: expr, $value: expr) => {
        static_assertions::const_assert_eq!($value, STEP_COUNT);
        static_assertions::const_assert_eq!((MAX_DIRTY_SIZE - MIN_DIRTY_SIZE_ROUNDED) % STEP_SIZE, 0);
        if MIN_DIRTY_SIZE != MIN_DIRTY_SIZE_ROUNDED {
            const $index: usize = MIN_DIRTY_SIZE;
            $inner
        }

        seq_macro::seq!(I in 0..$value {
            {
                const $index: usize = I * STEP_SIZE + MIN_DIRTY_SIZE_ROUNDED;
                $inner
            }
        });
    };
}

macro_rules! for_dirty_size {
    ($index: ident, $inner: expr) => {
        // the third argument has to be equal to STEP_COUNT!

        // because of the size of the metadata
        // STEP_COUNT has a different value for different target platforms!
        #[cfg(target_pointer_width = "32")]
        for_dirty_size_impl!($index, $inner, 121);

        #[cfg(target_pointer_width = "64")]
        #[cfg(test)]
        for_dirty_size_impl!($index, $inner, 113);

        #[cfg(target_pointer_width = "64")]
        #[cfg(not(test))]
        for_dirty_size_impl!($index, $inner, 114);
    };
}

pub(crate) struct DirtySizePersistLatencyRunner;

impl BenchmarkRunner for DirtySizePersistLatencyRunner {

    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_dirty_size_persist_latency {
            if MIN_DIRTY_SIZE != MIN_DIRTY_SIZE_ROUNDED {
                iteration_count += 2;
            }
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
    
        if options.run_dirty_size_persist_latency {
            let mut trigger = TRIGGER::new(trigger_persist);

            for_dirty_size!(DIRTY_SIZE, {
                handle_curr_iteration();

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

//                const REST: usize = remaining_dirty_size(REM_DIRTY_SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY);
//                println!("obj_cnt: {}, dirty_objects: {}, rest: {}, rest dirty?: {}, DIRTYREST: {}", OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY, REST);

                let bench: PersistLatencyBenchmark<
                    BUF_SIZE,
                    RESIDENT_CUTOFF_SIZE,
                    REM_OBJ_SIZE,
                    VNV_HEAP_RAM_OVERHEAD
                > = PersistLatencyBenchmark::new::<S>(DIRTY_SIZE, &mut heap, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_DIRTY, "max_objects_persist_latency_dirty_size");
                bench.run_benchmark::<TIMER, TRIGGER>(run_options, get_ticks, &mut trigger);
            });
            for_dirty_size!(DIRTY_SIZE, {
                handle_curr_iteration();

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
//                const REST: usize = remaining_dirty_size(REM_DIRTY_SIZE, BUF_SIZE - RESIDENT_CUTOFF_SIZE, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY);
//                println!("obj_cnt: {}, dirty_objects: {}, rest: {}, rest dirty?: {}, DIRTYREST: {}", OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_SIZE, REM_OBJ_DIRTY, REST);


                let bench: PersistLatencyBenchmark<
                    BUF_SIZE,
                    RESIDENT_CUTOFF_SIZE,
                    REM_OBJ_SIZE,
                    VNV_HEAP_RAM_OVERHEAD
                > = PersistLatencyBenchmark::new::<S>(DIRTY_SIZE, &mut heap, OBJ_CNT, DIRTY_NORMAL_OBJECTS, REM_OBJ_DIRTY, "max_dirty_persist_latency_dirty_size");
                bench.run_benchmark::<TIMER, TRIGGER>(run_options, get_ticks, &mut trigger);
            });
        }

    }
}
