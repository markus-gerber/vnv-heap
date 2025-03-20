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

extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;

use std::{array, ptr::slice_from_raw_parts_mut};

use rand::{rngs::SmallRng, RngCore, SeedableRng};

use spi_fram_storage::MB85RS4MTFramStorageModule;
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
    },
    vnv_persist_all, VNVConfig, VNVHeap,
};

#[no_mangle]
pub extern "C" fn persist() {
    unsafe {
        vnv_persist_all();
    }
}

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Warn);

    type TestType = [u8; 10];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    // configure vNVHeap
    let storage = unsafe { MB85RS4MTFramStorageModule::new() }.unwrap();

    let config = VNVConfig {
        max_dirty_bytes: 600,
    };
    let mut buffer = [0u8; 1000];

    // init vNVHeap
    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<19>,
        DefaultObjectManagementModule,
        MB85RS4MTFramStorageModule,
    > = VNVHeap::new(
        &mut buffer,
        storage,
        LinkedListAllocatorModule::new(),
        config,
        |base_ptr, size| {
            print!("clearing buffer... ");

            // clear vNVHeap's managed memory region
            let buffer = unsafe {
                slice_from_raw_parts_mut(base_ptr, size).as_mut()
            }.unwrap();
            buffer.fill(0);

            println!("ok");
        },
    )
    .unwrap();

    const SEED: u64 = 5446535461589659585;
    const OBJECT_COUNT: usize = 200;
    const ITERATION_MULTIPLIER: usize = 5000;

    println!("starting tests...");
    loop {
        let mut rand = SmallRng::seed_from_u64(SEED);

        let mut objects = vec![];
        let mut check_states = vec![];

        macro_rules! allocate {
            () => {
                let data = rand_data(&mut rand);

                objects.push(heap.allocate(data.clone()).unwrap());
                check_states.push(data);
            };
        }

        macro_rules! single_test {
            () => {
                let i = rand.next_u32() as usize % objects.len();
                let test_type = rand.next_u32() % 10;
                if test_type == 0 {
                    // get mut and change data
                    let mut mut_ref = objects[i].get_mut().unwrap();
                    assert_eq!(*mut_ref, check_states[i]);

                    let data = rand_data(&mut rand);
                    *mut_ref = data;
                    check_states[i] = data;
                } else if test_type < 2 {
                    // get mut and dont change data
                    let mut_ref = objects[i].get_mut().unwrap();
                    assert_eq!(*mut_ref, check_states[i]);
                } else {
                    // get ref
                    let immut_ref = objects[i].get().unwrap();
                    assert_eq!(*immut_ref, check_states[i]);
                }
            };
        }

        // start allocating some first objects
        for _ in 0..OBJECT_COUNT / 3 {
            allocate!();
        }

        // start testing
        for _ in 0..ITERATION_MULTIPLIER * 2 {
            single_test!();
        }

        log::warn!("-> Finished Test 1/3!");

        // test again
        for _ in 0..ITERATION_MULTIPLIER * 10 {
            single_test!();
        }

        log::warn!("-> Finished Test 2/3!");

        // start allocating last objects
        for _ in 0..(OBJECT_COUNT - objects.len()) {
            allocate!();
        }

        // test again
        for _ in 0..ITERATION_MULTIPLIER * 10 {
            single_test!();
        }

        log::warn!("-> Finished Test 3/3! Rerunning tests...");
    }
}
