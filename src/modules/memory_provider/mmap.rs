use std::ptr::null_mut;

use libc::{c_void, mmap, munmap, sysconf, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_READ, PROT_WRITE, _SC_PAGE_SIZE};

use super::MemoryProviderModule;


pub struct MMapMemoryProvider;

impl MemoryProviderModule for MMapMemoryProvider {
    unsafe fn map_block(size: usize) -> *mut u8 {
        let base_ptr = unsafe {
            mmap(
                null_mut(),
                size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if base_ptr == MAP_FAILED {
            panic!("map failed");
        }

        base_ptr as *mut u8
    }

    unsafe fn unmap_block(ptr: *mut u8, size: usize) {
        let code = unsafe {
            munmap(ptr as *mut c_void, size)
        };

        if code != 0 {
            println!("ERROR: Could not unmap page(s)");
        }
    }

    fn min_size() -> usize {
        unsafe { sysconf(_SC_PAGE_SIZE) as usize }
    }
}