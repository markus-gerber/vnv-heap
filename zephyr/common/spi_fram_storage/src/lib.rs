#[cfg(debug_assertions)]
pub use xxhash_rust::xxh32 as xxhash;

mod mb85rs4mt_fram_storage;
mod mb85rs64v_fram_storage;

pub use mb85rs4mt_fram_storage::*;
pub use mb85rs64v_fram_storage::*;
