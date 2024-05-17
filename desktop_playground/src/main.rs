use env_logger::{Builder, Env};
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::FilePersistentStorageModule,
    },
    VNVConfig, VNVHeap,
};

fn main() {
    Builder::from_env(Env::default())
        .filter_level(log::LevelFilter::Trace)
        .format_module_path(false)
        .init();

    let storage = FilePersistentStorageModule::new("test.data".to_string(), 4096).unwrap();
    let config = VNVConfig {
        max_dirty_bytes: 100,
    };
    let mut buffer = [0u8; 512];

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<16>,
        FilePersistentStorageModule,
    > = VNVHeap::new(&mut buffer, storage, config).unwrap();

    let mut obj = heap.allocate::<u32>(10).unwrap();

    {
        let obj_ref = obj.get().unwrap();

        println!("data: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj.get_mut().unwrap();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj.get().unwrap();

        println!("data: {}", *obj_ref);
    }

    let mut obj2 = heap.allocate::<u32>(1000).unwrap();

    {
        let obj_ref = obj2.get().unwrap();

        println!("data2: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj2.get_mut().unwrap();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj2.get().unwrap();

        println!("data2: {}", *obj_ref);
    }

    {
        let obj_ref = obj.get().unwrap();

        println!("data: {}", *obj_ref);
    }
}
