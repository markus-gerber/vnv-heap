#[macro_use]
extern crate log;

extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;

use core::mem::size_of;
use spi_fram_storage::MB85RS4MTFramStorageModule;
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
    },
    VNVConfig, VNVHeap, VNVObject, VNVRef, VNVMutRef
};

#[no_mangle]
pub extern "C" fn rust_main() {
    type A = LinkedListAllocatorModule;
    type N = NonResidentBuddyAllocatorModule<19>;
    type M = DefaultObjectManagementModule;
    type S = MB85RS4MTFramStorageModule;
    
    println!("############# MODULE SIZES #############");

    print_size::<A>();
    print_size::<N>();
    print_size::<M>();
    print_size::<S>();

    println!("########## STACK STRUCT SIZES ##########");

    print_size::<VNVHeap<A, N, M, S>>();

    assert_eq!(
        size_of::<VNVObject<u8, A, N, M>>(),
        size_of::<VNVObject<[u8; 100], A, N, M>>(),
        "The size of VNVObject should be independent of the underlying data!"
    );

    print_size::<VNVObject<u8, A, N, M>>();

    assert_eq!(
        size_of::<VNVRef<u8,A, N, M>>(),
        size_of::<VNVRef<[u8; 100], A, N, M>>(),
        "The size of VNVRef should be independent of the underlying data!"
    );

    print_size::<VNVRef<u8, A, N, M>>();

    assert_eq!(
        size_of::<VNVMutRef<u8, A, N, M>>(),
        size_of::<VNVMutRef<[u8; 100], A, N, M>>(),
        "The size of VNVMutRef should be independent of the underlying data!"
    );

    print_size::<VNVMutRef<u8, A, N, M>>();

    println!("############# BUFFER SIZES #############");

    let layout_info = VNVHeap::<A, N, M, S>::get_layout_info();

    println!("Buffer Cutoff\n-> {} bytes", layout_info.cutoff_size);
    println!("Resident Object Metadata\n-> {} bytes", layout_info.resident_object_metadata);
    println!("Resident Object Dirty Size\n-> {} bytes", layout_info.object_dirty_size);
    println!("Persist Access Point Size\n-> {} bytes", layout_info.persist_access_point_size);

    println!("############### FINISHED ###############")
}

fn print_size<T>() {
    let size = size_of::<T>();
    println!("{}\n-> {} bytes", std::any::type_name::<T>(), size);
}
