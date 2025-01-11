// collection of benchmarks to measure critical sections (during locked internal RAM heap + during IO)

mod util;
use util::*;

mod bench;
use bench::*;

mod sections;
use sections::*;

use std::mem::{align_of, size_of};

use super::*;
use crate::{
    benchmarks::{BenchmarkRunOptions, BenchmarkRunner, RunAllBenchmarkOptions, Timer}, calc_resident_buf_cutoff_size, modules::{object_management::DefaultObjectManagementModule, persistent_storage::DummyStorageModule}, resident_object_manager::resident_object_metadata::ResidentObjectMetadata, util::round_up_to_nearest, VNVObject
};

use super::{GetCurrentTicks, PersistTrigger, PersistentStorageModule};

type A = LinkedListAllocatorModule;
type N = NonResidentBuddyAllocatorModule<19>;
type M = DefaultObjectManagementModule;

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


const STEP_SIZE: usize = 32;

const MIN_BUFFER_SIZE: usize = 512 - VNV_HEAP_RAM_OVERHEAD;
const MAX_BUFFER_SIZE: usize = 4 * 1024 - VNV_HEAP_RAM_OVERHEAD;

const STEP_COUNT: usize = (MAX_BUFFER_SIZE - MIN_BUFFER_SIZE) / STEP_SIZE + 1;

macro_rules! for_buffer_size_impl {
    ($index: ident, $inner: expr) => {
        static_assertions::const_assert_eq!((MAX_BUFFER_SIZE - MIN_BUFFER_SIZE) % STEP_SIZE, 0);
        for x in 0..STEP_COUNT {
            let $index: usize = x * STEP_SIZE + MIN_BUFFER_SIZE;
            $inner
        }
    };
}

macro_rules! for_buffer_size {
    ($index: ident, $inner: expr) => {
        for_buffer_size_impl!($index, $inner);
    };
}


pub(crate) struct LockedWCETRunner;

impl BenchmarkRunner for LockedWCETRunner {
    fn get_iteration_count(options: &RunAllBenchmarkOptions) -> usize {
        let mut iteration_count = 0;
        if options.run_locked_wcet_benchmarks {
            iteration_count += 1;
            iteration_count += STEP_COUNT;
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
        _get_ticks: GetCurrentTicks,
    ) {
        assert_eq!(
            RESIDENT_CUTOFF_SIZE,
            calc_resident_buf_cutoff_size::<A, S>(),
            "cutoff size has to match"
        );

        if options.run_locked_wcet_benchmarks {
            let mut buffer = [0u8; MAX_BUFFER_SIZE];

            for_buffer_size!(buffer_size, {
                handle_curr_iteration();
                let mut a = A::new();
                let mut storage = get_storage();
                let executor = ObjectManager1LockedWCETExecutor::<TIMER>::new(&mut buffer[0..buffer_size], size_of::<usize>());
                let bench = LockedWCETBenchmark::new(&mut storage, &mut a, executor);
                bench.run_benchmark::<TIMER>(run_options);
            });
            
            {
                handle_curr_iteration();
                let mut a = A::new();
                let mut storage = get_storage();
                let mut buffer = [0u8; 4];
                let executor = StorageLockedWCETExecutor::<TIMER>::new(&mut buffer);
                let bench = LockedWCETBenchmark::new(&mut storage, &mut a, executor);
                bench.run_benchmark::<TIMER>(run_options);    
            }
        }
    }
}
