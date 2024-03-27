mod allocation_options;
mod vnv_heap;
mod vnv_mut_ref;
mod vnv_heap_manager;
mod vnv_meta_store_item;
mod vnv_meta_store;
mod vnv_object;
mod vnv_ref;
mod modules;
mod util;

use modules::{allocator::buddy::BuddyAllocatorModule, page_replacement::EmptyPageReplacementModule, page_storage::mmap::MMapPageStorageModule};
use vnv_heap::VNVHeap;

fn main() {
    let storage = MMapPageStorageModule::new("test.data").unwrap();

    let heap: VNVHeap<BuddyAllocatorModule<16>, EmptyPageReplacementModule, MMapPageStorageModule> = VNVHeap::new(EmptyPageReplacementModule, storage);

    let mut obj = heap.allocate::<u32>(10);

    {
        let obj_ref = obj.get();

        println!("data: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj.get_mut();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj.get();

        println!("data: {}", *obj_ref);
    }

}
