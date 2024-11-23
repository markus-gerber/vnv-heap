use std::{
    array,
    ptr::{null_mut, slice_from_raw_parts_mut},
};

use rand::{rngs::SmallRng, RngCore, SeedableRng};

use crate::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
    }, test::get_test_heap, vnv_persist_all, VNVObject
};

#[test]
fn test_persist_all_simple() {
    type TestType = [u8; 10];

    fn rand_data(rand: &mut SmallRng) -> TestType {
        array::from_fn(|_| rand.next_u32() as u8)
    }

    const BUFFER_SIZE: usize = 1200;
    let mut buffer = [0u8; BUFFER_SIZE];

    let heap = get_test_heap(
        "test_persist_all_simple",
        8 * 4096,
        &mut buffer,
        800,
        |base_ptr, size| {
            let buffer = unsafe { slice_from_raw_parts_mut(base_ptr, size).as_mut() }.unwrap();
            buffer.fill(0);

            // we can use print here because in our example this will only be called by the main thread
            println!("finished clearing buffer with {} bytes", size);
        },
    );

    const SEED: u64 = 5446535461589659585;

    let mut rand = SmallRng::seed_from_u64(SEED);

    let mut objects = vec![];
    let mut check_states = vec![];
    let mut resident: Vec<bool> = vec![];

    macro_rules! allocate {
        () => {
            let data = rand_data(&mut rand);

            objects.push(heap.allocate(data.clone()).unwrap());
            check_states.push(data);
            resident.push(false);
        };
    }

    fn update(
        objects: &mut Vec<
            VNVObject<
                [u8; 10],
                LinkedListAllocatorModule,
                NonResidentBuddyAllocatorModule<16>,
                DefaultObjectManagementModule,
            >,
        >,
        check_states: &mut Vec<[u8; 10]>,
        id: usize,
        rand: &mut SmallRng,
    ) {
        let data = rand_data(rand);

        let mut obj_ref = objects[id].get_mut().unwrap();
        *obj_ref = data.clone();
        drop(obj_ref);

        check_states[id] = data;
    }

    fn check_integrity(
        objects: &mut Vec<
            VNVObject<
                [u8; 10],
                LinkedListAllocatorModule,
                NonResidentBuddyAllocatorModule<16>,
                DefaultObjectManagementModule,
            >,
        >,
        check_states: &mut Vec<[u8; 10]>,
    ) {
        for (object, check_state) in objects.iter().zip(check_states.iter()) {
            let obj_ref = object.get().unwrap();
            assert_eq!(*obj_ref, *check_state)
        }
    }

    fn checked_persist(
        buffer_ptr: *mut u8,
        objects: &mut Vec<
            VNVObject<
                [u8; 10],
                LinkedListAllocatorModule,
                NonResidentBuddyAllocatorModule<16>,
                DefaultObjectManagementModule,
            >,
        >,
        check_states: &mut Vec<[u8; 10]>,
        resident: &mut Vec<bool>,
    ) {
        println!("checked");
        for i in 0..objects.len() {
            resident[i] = objects[i].is_resident()
        }

        let mut compare_buffer = [0u8; 1200];
        if !buffer_ptr.is_null() {
            let buffer = unsafe { slice_from_raw_parts_mut(buffer_ptr, 1200).as_mut() }.unwrap();
            for i in 0..buffer.len().min(compare_buffer.len()) {
                compare_buffer[i] = buffer[i];
            }
        }

        unsafe { vnv_persist_all() };

        if !buffer_ptr.is_null() {
            let buffer = unsafe { slice_from_raw_parts_mut(buffer_ptr, 1200).as_mut() }.unwrap();
            for i in 0..buffer.len().min(compare_buffer.len()) {
                assert_eq!(
                    compare_buffer[i], buffer[i],
                    "DOES NOT MATCH AT INDEX {}!\nOriginal Buffer: {:?}\nCurrent Buffer:  {:?}",
                    i, compare_buffer, buffer
                );
            }
        }

        for i in 0..objects.len() {
            assert_eq!(resident[i], objects[i].is_resident())
        }

        check_integrity(objects, check_states);
    }

    allocate!();

    checked_persist(null_mut(), &mut objects, &mut check_states, &mut resident);

    for _ in 0..100 {
        allocate!();
    }

    checked_persist(null_mut(), &mut objects, &mut check_states, &mut resident);

    for i in (0..objects.len()).step_by(3) {
        objects[i].get().unwrap();
    }

    checked_persist(null_mut(), &mut objects, &mut check_states, &mut resident);

    for i in [10, 23, 45, 1, 24, 10, 100] {
        update(&mut objects, &mut check_states, i, &mut rand);
    }

    checked_persist(null_mut(), &mut objects, &mut check_states, &mut resident);
}
