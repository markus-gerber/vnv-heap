#![no_std]

#[macro_use]
extern crate log;

extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;

use spi_fram_storage::SpiFramStorageModule;
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
    },
    VNVConfig, VNVHeap,
};

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Trace);

    let storage = unsafe { SpiFramStorageModule::new() }.unwrap();

    let config = VNVConfig {
        max_dirty_bytes: 100,
    };
    let mut buffer = [0u8; 100];

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<16>,
        DefaultObjectManagementModule,
        SpiFramStorageModule,
    > = VNVHeap::new(
        &mut buffer,
        storage,
        LinkedListAllocatorModule::new(),
        config,
        |_, _| {},
    )
    .unwrap();

    let mut obj = heap.allocate::<u32>(10).unwrap();

    {
        let obj_ref = obj.get().unwrap();

        info!("data: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj.get_mut().unwrap();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj.get().unwrap();

        info!("data: {}", *obj_ref);
    }

    let mut obj2 = heap.allocate::<u32>(1000).unwrap();

    {
        let obj_ref = obj2.get().unwrap();

        info!("data2: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj2.get_mut().unwrap();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj2.get().unwrap();

        info!("data2: {}", *obj_ref);
    }

    {
        let obj_ref = obj.get().unwrap();

        info!("data: {}", *obj_ref);
    }
}
