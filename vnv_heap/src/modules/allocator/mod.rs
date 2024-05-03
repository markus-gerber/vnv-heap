#[cfg(feature = "buddy_allocator")]
mod buddy;

#[cfg(feature = "linked_list_allocator")]
mod linked_list;

#[cfg(feature = "buddy_allocator")]
pub use buddy::BuddyAllocatorModule;
#[cfg(feature = "linked_list_allocator")]
pub use linked_list::LinkedListAllocatorModule;

use core::{alloc::Layout, ptr::NonNull};

pub trait AllocatorModule {
    /// Creates a new allocator module object.
    ///
    /// **Note**: It first will be initialized before it will be used
    fn new() -> Self;

    /// Initializes the allocator module with a memory area
    /// `[start, start+size)`
    unsafe fn init(&mut self, start: *mut u8, size: usize);

    /// Allocates new memory
    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()>;

    /// Deallocates memory
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout);
}
