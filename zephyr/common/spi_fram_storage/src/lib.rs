#[cfg(debug_assertions)]
mod xxhash;

mod spi_fram_storage;

pub use spi_fram_storage::*;
