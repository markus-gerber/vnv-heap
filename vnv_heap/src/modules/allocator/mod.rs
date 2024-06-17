#[cfg(feature = "buddy_allocator")]
mod buddy;

mod linked_list;

#[cfg(feature = "buddy_allocator")]
pub use buddy::BuddyAllocatorModule;

pub use linked_list::LinkedListAllocatorModule;

use core::{alloc::Layout, ptr::NonNull};

pub trait AllocatorModule {
    /// Initializes the allocator module with a memory area
    /// `[start, start+size)`
    unsafe fn init(&mut self, start: *mut u8, size: usize);

    /// Allocates new memory
    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()>;

    /// Deallocates memory
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout);

    /// Resets this module
    unsafe fn reset(&mut self);

    /// Allocates `layout` at the location of `ptr`
    unsafe fn allocate_at(&mut self, layout: Layout, ptr: *mut u8) -> Result<(), ()>;
}
