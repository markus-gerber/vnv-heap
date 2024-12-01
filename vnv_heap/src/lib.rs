mod allocation_identifier;
mod resident_object_manager;
mod persist_access_point;
mod shared_persist_lock;
mod vnv_config;
mod vnv_heap;
mod vnv_list;
mod vnv_list_mut_ref;
mod vnv_list_ref;
mod vnv_array;
mod vnv_array_mut_ref;
mod vnv_mut_ref;
mod vnv_object;
mod vnv_ref;
mod util;

#[cfg(test)]
mod test;

#[cfg(any(feature = "benchmarks", test))]
pub mod benchmarks;

pub use crate::vnv_heap::*;
pub use crate::vnv_object::VNVObject;
pub use crate::vnv_array::VNVArray;
pub use vnv_config::VNVConfig;
pub mod modules;
