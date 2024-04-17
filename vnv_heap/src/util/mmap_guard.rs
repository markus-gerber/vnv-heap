use core::{
    mem::{size_of, ManuallyDrop},
    ops::{Deref, DerefMut},
    ptr::null_mut,
};

use libc::{c_void, mmap, munmap, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_READ, PROT_WRITE};

use super::{ceil_div, get_page_size};

struct MMapGuard<T: MMapGuardInner + 'static> {
    inner: ManuallyDrop<&'static mut T>,
    size: usize
}

impl<T: MMapGuardInner> MMapGuard<T> {
    /// Maps a new page with the size `get_page_size()` and writes `T` to the start of this page.
    ///
    /// If finished, `T.use_remaining_data(...)` will be called.
    /// 
    /// `min_size` is the minimum size in bytes that is required 
    /// 
    /// ### Safety
    /// 
    /// `min_size` has to be greater or equal to the size that is needed to store `T`. 
    pub(crate) unsafe fn new(data: T, min_size: usize) -> Self {
        debug_assert!(size_of::<T>() <= min_size, "min_size should be big enough for T!");

        let page_size = get_page_size();
        let mmap_size = ceil_div(min_size, page_size) * page_size;

        if size_of::<T>() > mmap_size {
            panic!("Type T is too big!");
        }

        let base_ptr = unsafe {
            mmap(
                null_mut(),
                mmap_size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if base_ptr == MAP_FAILED {
            panic!("map failed");
        }

        let base_ptr = base_ptr as *mut u8;
        unsafe { core::ptr::write(base_ptr as *mut T, data) };

        // reference to newly initialized data
        let data = unsafe { (base_ptr as *mut T).as_mut().unwrap() };

        // notify T that the rest of the page can be used now
        let unused_ptr = unsafe { base_ptr.offset(size_of::<T>() as isize) };
        data.use_remaining_data(unused_ptr, mmap_size - size_of::<T>());

        let data_ref = unsafe {
            (base_ptr as *mut T).as_mut().unwrap()
        };

        Self {
            inner: ManuallyDrop::new(data_ref),
            size: mmap_size
        }
    }
}

impl<T: MMapGuardInner> MMapGuard<T> {
    pub(crate) fn size(&self) -> usize {
        self.size
    }
}

impl<T: MMapGuardInner> Deref for MMapGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: MMapGuardInner> DerefMut for MMapGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: MMapGuardInner> Drop for MMapGuard<T> {
    fn drop(&mut self) {
        let ptr = *self.inner as *mut T;

        unsafe {
            ManuallyDrop::drop(&mut self.inner);
        }

        // dropped all values, now unmap page
        let code = unsafe { munmap(ptr as *mut c_void, self.size) };

        if code != 0 {
            println!("ERROR: Could not unmap meta page");
        }
    }
}

pub(crate) trait MMapGuardInner {
    /// This will be called once `MMapGuard` has mapped a page and written `T` to the start.
    ///
    /// `ptr` is the first pointer after `T` and `remaining_size` is the amount of bytes that can still be used.
    /// So [ptr, ptr + remaining_size) is usable.
    fn use_remaining_data(&mut self, ptr: *mut u8, remaining_size: usize);
}
