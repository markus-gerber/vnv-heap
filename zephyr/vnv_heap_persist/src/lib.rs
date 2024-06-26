
extern crate zephyr_macros;
extern crate zephyr;
extern crate zephyr_logger;
extern crate zephyr_core;

use std::{
    array,
    ptr::slice_from_raw_parts_mut,
};

use rand::{rngs::SmallRng, RngCore, SeedableRng};

use spi_fram_storage::SpiFramStorageModule;
use vnv_heap::{vnv_persist_all, VNVConfig, VNVHeap, modules::{nonresident_allocator::NonResidentBuddyAllocatorModule, allocator::LinkedListAllocatorModule, object_management::DefaultObjectManagementModule}};

#[no_mangle]
pub extern "C" fn persist() {
    unsafe { vnv_persist_all(); }
}

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Warn);

    type TestType = [u8; 10];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    let storage = unsafe { SpiFramStorageModule::new() }.unwrap();
    
    let config = VNVConfig {
        max_dirty_bytes: 600
    };
    let mut buffer = [0u8; 1000];
    
    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<16>,
        DefaultObjectManagementModule
    > = VNVHeap::new(&mut buffer, storage, LinkedListAllocatorModule::new(), config, |base_ptr, size| {
        // TODO: is printing safe to communicate with UART? If not: make it safe
	    print!("clearing buffer... ");
        
        let buffer = unsafe { slice_from_raw_parts_mut(base_ptr, size).as_mut() }.unwrap();
        buffer.fill(0);

        // TODO: is printing safe to communicate with UART? If not: make it safe
	    println!("ok");
    }).unwrap();

    const SEED: u64 = 5446535461589659585;
    const OBJECT_COUNT: usize = 200;
    const ITERATION_MULTIPLIER: usize = 1000;

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
        for _ in 0..OBJECT_COUNT/3 {
            allocate!();
        }

        // start testing
        for _ in 0..ITERATION_MULTIPLIER * 2 {
            single_test!();
        }

        log::warn!("-> Finished Test 1!");

        let mut open_ref_obj = vec![];
        let mut open_refs = vec![];
        let mut open_muts = vec![];
        for _ in 0..10 {
            open_ref_obj.push(heap.allocate(rand_data(&mut rand)).unwrap());
        }
        for (i, obj) in open_ref_obj.iter_mut().enumerate() {
            if i % 2 == 0 {
                open_refs.push(obj.get().unwrap());
                open_refs.push(obj.get().unwrap());
                open_refs.push(obj.get().unwrap());
            } else {
                open_muts.push(obj.get_mut().unwrap());
            }
        }

        // test again
        for _ in 0..ITERATION_MULTIPLIER * 2 {
            single_test!();
        }

        log::warn!("-> Finished Test 2!");

        // drop open refs
        drop(open_refs);


        // test again
        for _ in 0..ITERATION_MULTIPLIER * 5 {
            single_test!();
        }

        log::warn!("-> Finished Test 3!");

        drop(open_muts);
        drop(open_ref_obj);

        // test again
        for _ in 0..ITERATION_MULTIPLIER * 10 {
            single_test!();
        }

        log::warn!("-> Finished Test 4!");

        // start allocating last objects
        for _ in 0..(OBJECT_COUNT - objects.len()) {
            allocate!();
        }

        // test again
        for _ in 0..ITERATION_MULTIPLIER * 10 {
            single_test!();
        }

        log::warn!("-> Finished Test 5!");
    }
}
