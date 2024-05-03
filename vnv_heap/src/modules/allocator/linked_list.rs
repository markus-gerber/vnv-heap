use core::{alloc::Layout, ptr::NonNull};

use super::AllocatorModule;
use linked_list_allocator::Heap;

/// Linked list allocator module that uses first fit
pub struct LinkedListAllocatorModule {
    inner: Heap,
}

impl AllocatorModule for LinkedListAllocatorModule {
    fn new() -> Self {
        Self {
            inner: Heap::empty(),
        }
    }

    unsafe fn init(&mut self, start: *mut u8, size: usize) {
        self.inner.init(start, size)
    }

    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
        self.inner.allocate_first_fit(layout)
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.inner.deallocate(ptr, layout)
    }
}
