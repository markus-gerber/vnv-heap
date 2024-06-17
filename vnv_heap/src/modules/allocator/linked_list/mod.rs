mod hole;
mod internal;

use core::{alloc::Layout, ptr::NonNull};

use super::AllocatorModule;
use internal::Heap;

/// Linked list allocator module that uses first fit
pub struct LinkedListAllocatorModule {
    inner: Heap,
}

impl AllocatorModule for LinkedListAllocatorModule {
    unsafe fn init(&mut self, start: *mut u8, size: usize) {
        self.inner.init(start, size)
    }

    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
        self.inner.allocate_first_fit(layout)
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.inner.deallocate(ptr, layout)
    }

    unsafe fn reset(&mut self) {
        self.inner = Heap::empty()
    }
    
    unsafe fn allocate_at(&mut self, layout: Layout, ptr: *mut u8) -> Result<(), ()> {
        self.inner.allocate_at(layout, ptr)
    }
}

impl LinkedListAllocatorModule {
    pub fn new() -> Self {
        Self {
            inner: Heap::empty()
        }
    }
}