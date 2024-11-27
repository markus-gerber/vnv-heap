use std::{array, vec};
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use vnv_heap::{VNVConfig, VNVHeap, modules::{nonresident_allocator::NonResidentBuddyAllocatorModule, allocator::LinkedListAllocatorModule, object_management::DefaultObjectManagementModule}};
use spi_fram_storage::MB85RS4MTFramStorageModule;

pub fn test_heap_persistency() {
    type TestType = [u8; 10];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    let storage = unsafe { MB85RS4MTFramStorageModule::new() }.unwrap();
    
    let config = VNVConfig {
        max_dirty_bytes: 1000
    };
    let mut buffer = [0u8; 1000];
    
    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<19>,
        DefaultObjectManagementModule,
        MB85RS4MTFramStorageModule
    > = VNVHeap::new(&mut buffer, storage, LinkedListAllocatorModule::new(), config, |_, _| {}).unwrap();

    const SEED: u64 = 5446535461589659585;
    const OBJECT_COUNT: usize = 200;
    const ITERATION_MULTIPLIER: usize = 10000;

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
                assert_eq!(*mut_ref, check_states[i], "test_type {}", test_type);
                
                let data = rand_data(&mut rand);
                *mut_ref = data;
                check_states[i] = data;
            } else if test_type < 2 {
                // get mut and dont change data
                let mut_ref = objects[i].get_mut().unwrap();
                assert_eq!(*mut_ref, check_states[i], "test_type {}", test_type);
            } else {
                // get ref
                let immut_ref = objects[i].get().unwrap();
                assert_eq!(*immut_ref, check_states[i], "test_type {}", test_type);
            }
        };
    }

    log::warn!("Starting Tests...");

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
