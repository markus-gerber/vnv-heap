mod allocation_options;
mod modules;
mod util;
mod vnv_heap;
mod vnv_heap_metadata;
mod vnv_meta_store;
mod vnv_meta_store_item;
mod vnv_mut_ref;
mod vnv_object;
mod vnv_ref;
mod vnv_resident_heap;
mod vnv_resident_heap_manager;

use env_logger::{Builder, Env};
use modules::{
    allocator::buddy::BuddyAllocatorModule, memory_provider::mmap::MMapMemoryProvider,
    page_replacement::EmptyPageReplacementModule, page_storage::mmap::MMapPageStorageModule,
};
use vnv_heap::VNVHeap;
use vnv_resident_heap_manager::VNVResidentHeapManagerConfig;

fn main() {
    Builder::from_env(Env::default())
        .filter_level(log::LevelFilter::Trace)
        .format_module_path(false)
        .init();

    let storage = MMapPageStorageModule::new("test.data").unwrap();
    let config = VNVResidentHeapManagerConfig {
        max_dirty_size: 4096 * 4,
        max_resident_size: 4096 * 8,
    };

    let heap: VNVHeap<
        BuddyAllocatorModule<16>,
        EmptyPageReplacementModule,
        MMapPageStorageModule,
        MMapMemoryProvider,
    > = VNVHeap::new(EmptyPageReplacementModule, storage, config);

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
