mod allocation_options;
mod allocation_identifier;
mod resident_object_manager;
mod persist_access_point;
mod vnv_config;
mod vnv_heap;
mod vnv_mut_ref;
mod vnv_object;
mod vnv_ref;
mod util;

#[cfg(test)]
mod test;

#[cfg(feature = "benchmarks")]
pub mod benchmarks;

pub use crate::vnv_heap::VNVHeap;
pub use crate::vnv_object::VNVObject;
pub use vnv_config::VNVConfig;
pub mod modules;
