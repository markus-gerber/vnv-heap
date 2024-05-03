mod allocation_options;
mod allocation_identifier;
mod resident_object;
mod resident_object_manager;
mod vnv_config;
mod vnv_heap;
mod vnv_mut_ref;
mod vnv_object;
mod vnv_ref;
mod util;

pub use crate::vnv_heap::VNVHeap;
pub use vnv_config::VNVConfig;
pub mod modules;
