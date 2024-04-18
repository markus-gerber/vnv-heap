#[cfg(feature = "use_libc")]
pub mod mmap;

/// A module that provides dynamic memory blocks during runtime
/// (e.g. trough `mmap`)
pub trait MemoryProviderModule {
    /// Returns a new memory block with `size` bytes.
    /// 
    /// It is guaranteed that `size` is a multiple of `min_size()`.
    unsafe fn map_block(size: usize) -> *mut u8;

    /// Removes the memory block.
    /// 
    /// It is guaranteed that `size` is a multiple of `min_size()`.
    unsafe fn unmap_block(ptr: *mut u8, size: usize);

    /// Gets the minimum size of a memory block.
    /// This should be the same value on every call or things might break.
    fn min_size() -> usize;
}