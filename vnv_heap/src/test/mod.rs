use crate::{modules::{persistent_storage::{test::get_test_storage, FilePersistentStorageModule}, allocator::LinkedListAllocatorModule, nonresident_allocator::NonResidentBuddyAllocatorModule}, VNVHeap};

mod persistency;

#[cfg(not(no_std))]
fn get_test_heap<'a>(test_name: &str, size: usize, resident_buffer: &'a mut [u8], dirty_size: usize) -> VNVHeap<'a, LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, FilePersistentStorageModule> {
    use crate::VNVConfig;

    let storage = get_test_storage(test_name, size);
    VNVHeap::new(resident_buffer, storage, VNVConfig {
        max_dirty_bytes: dirty_size
    }).unwrap()
}

#[cfg(no_std)]
fn get_test_heap(test_name: &str, resident_buffer: &mut [u8]) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>, FilePersistentStorageModule> {
    todo!("not implemented")
}