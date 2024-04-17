
use env_logger::{Builder, Env};
use vnv_heap::modules::{
    allocator::buddy::BuddyAllocatorModule, memory_provider::mmap::MMapMemoryProvider,
    page_replacement::EmptyPageReplacementModule, page_storage::file_mmap::FileMMapPageStorageModule,
};
use vnv_heap::{VNVHeap, VNVResidentHeapManagerConfig};

fn main() {
    Builder::from_env(Env::default())
        .filter_level(log::LevelFilter::Trace)
        .format_module_path(false)
        .init();

    let storage = FileMMapPageStorageModule::new("test.data").unwrap();
    let config = VNVResidentHeapManagerConfig {
        max_dirty_size: 4096 * 4,
        max_resident_size: 4096 * 8,
    };

    let heap: VNVHeap<
        BuddyAllocatorModule<16>,
        EmptyPageReplacementModule,
        FileMMapPageStorageModule,
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
