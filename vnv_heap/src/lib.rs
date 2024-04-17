mod allocation_options;
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

pub use crate::vnv_heap::VNVHeap;
pub use vnv_resident_heap_manager::VNVResidentHeapManagerConfig;
pub mod modules;
