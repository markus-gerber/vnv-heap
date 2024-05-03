use core::{alloc::Layout, ptr::NonNull};

use super::AllocatorModule;
use buddy_system_allocator::Heap;

/// Buddy allocator module
pub struct BuddyAllocatorModule<const ORDER: usize> {
    inner: Heap<ORDER>,
}

impl<const ORDER: usize> AllocatorModule for BuddyAllocatorModule<ORDER> {
    fn new() -> Self {
        Self { inner: Heap::new() }
    }

    unsafe fn init(&mut self, start: *mut u8, size: usize) {
        self.inner.init(start as usize, size)
    }

    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
        self.inner.alloc(layout)
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.inner.dealloc(ptr, layout)
    }
}
