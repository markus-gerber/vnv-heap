use crate::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
        persistent_storage::test::get_test_storage,
        persistent_storage::FilePersistentStorageModule
    },
    VNVHeap,
};

mod benchmarks;
mod persist_all;
mod persistency;
mod unload;

#[cfg(not(no_std))]
pub(crate) fn get_test_heap<'a>(
    test_name: &str,
    size: usize,
    resident_buffer: &'a mut [u8],
    dirty_size: usize,
    persist_handler: fn(*mut u8, usize) -> ()
) -> VNVHeap<
    'a,
    LinkedListAllocatorModule,
    NonResidentBuddyAllocatorModule<16>,
    DefaultObjectManagementModule,
    FilePersistentStorageModule
> {
    use crate::VNVConfig;

    let storage = get_test_storage(test_name, size);

    VNVHeap::new(
        resident_buffer,
        storage,
        LinkedListAllocatorModule::new(),
        VNVConfig {
            max_dirty_bytes: dirty_size,
        },
        persist_handler
    )
    .unwrap()
}

#[cfg(no_std)]
pub(crate) fn get_test_heap(
    test_name: &str,
    resident_buffer: &mut [u8],
) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>> {
    panic!("not implemented")
}
