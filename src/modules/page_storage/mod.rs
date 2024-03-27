pub mod mmap;

use std::ptr::NonNull;

// TODO: think about which functions need to be unsafe...
pub trait PageStorageModule {
    /// Map data region [offset, offset + size)
    /// 
    /// **Note**: `offset`` and `size` has to be multiples of a page size (e.g. sysconf(_SC_PAGESIZE))
    unsafe fn map(&mut self, offset: u64, size: usize) -> Result<NonNull<u8>, ()>;

    /// Create new data region in storage, that can be mapped later
    ///
    /// Returns the offset of the created region, that can be used to map it
    /// 
    /// **Note**: `size` has to be multiples of a page size (e.g. sysconf(_SC_PAGESIZE))
    fn add_new_region(&mut self, size: usize) -> Result<u64, ()>;

    /// Creates a new storage region and automatically maps it into memory
    /// 
    /// **Note**: `size` has to be multiples of a page size (e.g. sysconf(_SC_PAGESIZE))
    unsafe fn map_new_region(&mut self, size: usize) -> Result<NonNull<u8>, ()> {
        let offset = self.add_new_region(size)?;

        self.map(offset, size)
    }

    /// Unmaps a specific memory region without syncing,
    /// because unmap is only called on already synced pages
    /// 
    /// **Note**: `size` has to be multiples of a page size (e.g. sysconf(_SC_PAGESIZE))
    unsafe fn unmap(&mut self, pointer: NonNull<u8>, size: usize) -> Result<(), ()>;

    /// Syncs all changes back to non volatile storage
    /// 
    /// **Note**: `size` has to be multiples of a page size (e.g. sysconf(_SC_PAGESIZE))
    fn persist(&mut self, pointer: NonNull<u8>, size: usize) -> Result<(), ()>;
}