use core::{
    array,
    ptr::{null, null_mut},
};
use std::sync::atomic::AtomicBool;

use try_lock::TryLock;

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::{BuddyAllocatorModule, LinkedListAllocatorModule},
        nonresident_allocator::{NonResidentAllocatorModule, NonResidentBuddyAllocatorModule},
        object_management::DefaultObjectManagementModule,
        persistent_storage::{test::get_test_storage, PersistentStorageModule},
    },
    resident_object_manager::{calc_resident_obj_layout_static, calc_user_data_offset_static, resident_list::ResidentList}, shared_persist_lock::SharedPersistLock,
};

use super::ResidentObjectManager;

// test that dirty size will return to
// its initial value once no objects are resident anymore
#[test]
fn test_release_dirty_size() {
    const INITIAL_DIRTY_SIZE: usize = 300;
    const STORAGE_SIZE: usize = 4096 * 8;
    const OBJ_COUNT: usize = 200;
    type TestObj = [u8; 20];

    let mut buffer = [0u8; 500];
    let mut storage = get_test_storage("rom_test_release_dirty_size", STORAGE_SIZE);
    let mut non_resident_alloc = NonResidentBuddyAllocatorModule::<16>::new();

    let mut resident_list = ResidentList::new();

    let mut heap = LinkedListAllocatorModule::new();

    let lock = TryLock::new(());
    let persist_queued = AtomicBool::new(false);
    let shared_heap_lock: SharedPersistLock<*mut LinkedListAllocatorModule> =
        SharedPersistLock::new(&mut heap, &persist_queued, &lock);

    let mut manager =
        ResidentObjectManager::<LinkedListAllocatorModule, DefaultObjectManagementModule>::new(
            &mut buffer,
            INITIAL_DIRTY_SIZE,
            &mut resident_list,
            shared_heap_lock
        )
        .unwrap();

    non_resident_alloc
        .init(0, STORAGE_SIZE, &mut storage)
        .unwrap();

    let initial_data: TestObj = [0u8; 20];
    let offset_list: [usize; OBJ_COUNT] = array::from_fn(|_| {
        let offset = non_resident_alloc
            .allocate(calc_resident_obj_layout_static::<TestObj>(), &mut storage)
            .unwrap();

        // zero out space
        storage.write(offset + calc_user_data_offset_static::<TestObj>(), &initial_data).unwrap();

        offset
    });

    for (offset, i) in offset_list.iter().zip(0..) {
        if i % 3 == 0 {
            unsafe {
                manager
                    .get_ref(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();

                manager.release_ref(
                    &AllocationIdentifier::<TestObj>::from_offset(*offset),
                );
            }
        } else if i % 4 == 0 {
            manager
                .drop(
                    &AllocationIdentifier::<TestObj>::from_offset(*offset),
                    &mut storage,
                )
                .unwrap();
        } else {
            unsafe {
                manager
                    .get_mut(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();

                manager.release_mut(
                    &AllocationIdentifier::<TestObj>::from_offset(*offset),
                );
            }
        }
    }

    for offset in offset_list {
        manager
            .drop(
                &AllocationIdentifier::<TestObj>::from_offset(offset),
                &mut storage,
            )
            .unwrap();
    }

    assert_eq!(manager.remaining_dirty_size, INITIAL_DIRTY_SIZE);
}

// test that objects which have open references
// remain in RAM at all circumstances
#[test]
fn test_remain_resident() {
    const INITIAL_DIRTY_SIZE: usize = 300;
    const STORAGE_SIZE: usize = 4096 * 8;
    const OBJ_COUNT: usize = 200;
    type TestObj = [u8; 20];

    let mut buffer = [0u8; 800];
    let mut storage = get_test_storage("rom_test_remain_resident", STORAGE_SIZE);
    let mut non_resident_alloc = NonResidentBuddyAllocatorModule::<16>::new();

    let mut resident_list = ResidentList::new();

    let mut heap = BuddyAllocatorModule::<16>::new();

    let lock = TryLock::new(());
    let persist_queued = AtomicBool::new(false);
    let shared_heap_lock: SharedPersistLock<*mut BuddyAllocatorModule<16>> =
        SharedPersistLock::new(&mut heap, &persist_queued, &lock);

    let mut manager =
        ResidentObjectManager::<BuddyAllocatorModule<16>, DefaultObjectManagementModule>::new(
            &mut buffer,
            INITIAL_DIRTY_SIZE,
            &mut resident_list,
            shared_heap_lock,

        )
        .unwrap();

    non_resident_alloc
        .init(0, STORAGE_SIZE, &mut storage)
        .unwrap();

    let initial_data: TestObj = [0u8; 20];
    let offset_list: [usize; OBJ_COUNT] = array::from_fn(|_| {
        let offset = non_resident_alloc
            .allocate(calc_resident_obj_layout_static::<TestObj>(), &mut storage)
            .unwrap();

        // zero out space
        storage.write(offset + calc_user_data_offset_static::<TestObj>(), &initial_data).unwrap();

        offset
    });

    let ref_offsets = [offset_list[2], offset_list[100]];
    let mut ref_ptrs = [null(), null(), null(), null()];
    let mut_offsets = [offset_list[10], offset_list[67]];
    let mut mut_ptrs = [null_mut(), null_mut()];

    macro_rules! check_resident {
        () => {
            unsafe {
                ref_offsets
                    .iter()
                    .chain(mut_offsets.iter())
                    .for_each(|curr_offset| {
                        assert!(
                            manager
                                .find_element_mut(&AllocationIdentifier::<TestObj>::from_offset(
                                    *curr_offset
                                ))
                                .is_some(),
                            "Element with offset {} should be resident",
                            curr_offset
                        );
                    })
            }
        };
    }

    // first iteration, get (mutable) references without releasing them
    for offset in offset_list.iter() {
        if let Some((_, i)) = ref_offsets
            .iter()
            .zip(0..)
            .find(|(cur_off, _)| *cur_off == offset)
        {
            unsafe {
                let ptr = manager
                    .get_ref(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();
                ref_ptrs[i] = ptr;
            }
        } else if let Some((_, i)) = mut_offsets
            .iter()
            .zip(0..)
            .find(|(cur_off, _)| *cur_off == offset)
        {
            unsafe {
                let ptr = manager
                    .get_mut(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();
                mut_ptrs[i] = ptr;
            }
        } else {
            unsafe {
                manager
                    .require_resident(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();
            }
        }
    }

    check_resident!();

    // second iteration, get another round of immutable references
    for offset in offset_list.iter() {
        if let Some((_, i)) = ref_offsets
            .iter()
            .zip(0..)
            .find(|(cur_off, _)| *cur_off == offset)
        {
            unsafe {
                let ptr = manager
                    .get_ref(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();
                ref_ptrs[i + ref_offsets.len()] = ptr;
            }
        } else {
            unsafe {
                manager
                    .require_resident(
                        &AllocationIdentifier::<TestObj>::from_offset(*offset),
                        &mut storage,
                    )
                    .unwrap();
            }
        }

        check_resident!();
    }

    // release first round of references
    for i in 0..ref_offsets.len() {
        unsafe {
            manager.release_ref(
                &AllocationIdentifier::<TestObj>::from_offset(ref_offsets[i]),
            );
        }
    }

    check_resident!();

    // third iteration, make sure our objects still remain resident
    for offset in offset_list.iter() {
        unsafe {
            manager
                .require_resident(
                    &AllocationIdentifier::<TestObj>::from_offset(*offset),
                    &mut storage,
                )
                .unwrap();
        }

        check_resident!();
    }

    // remove references
    for i in 0..ref_offsets.len() {
        unsafe {
            manager.release_ref(
                &AllocationIdentifier::<TestObj>::from_offset(ref_offsets[i]),
            );
        }
    }

    // remove mutable references
    for i in 0..mut_offsets.len() {
        unsafe {
            manager.release_mut(
                &AllocationIdentifier::<TestObj>::from_offset(mut_offsets[i])
            );
        }
    }

    // make all objects non resident
    for offset in offset_list.iter() {
        manager
            .drop(
                &AllocationIdentifier::<TestObj>::from_offset(*offset),
                &mut storage,
            )
            .unwrap();
    }

    assert_eq!(manager.resident_object_count, 0);
    assert!(manager.resident_list.is_empty());
}
