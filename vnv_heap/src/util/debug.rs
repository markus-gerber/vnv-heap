use core::alloc::{GlobalAlloc, Layout, System};

use libc::{c_void, write, STDOUT_FILENO};

struct DebugAllocator;

// notice if default heap is used
unsafe impl GlobalAlloc for DebugAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let s = b"alloc\n";
        unsafe {
            write(STDOUT_FILENO, s.as_ptr() as *const c_void, s.len());
        }
        
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: DebugAllocator = DebugAllocator;
