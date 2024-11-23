mod buddy;
mod linked_list;

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

    #[cfg(debug_assertions)]
    #[allow(unused)]
    fn debug(&mut self);
}

#[cfg(test)]
mod test {
    use std::{alloc::Layout, ptr::NonNull};

    use super::AllocatorModule;

    fn convert_ptr(ptr: NonNull<u8>, diff: isize) -> *mut u8 {
        ((ptr.as_ptr() as isize) + diff) as *mut u8
    }

    #[repr(C, align(128))]
    struct Buffer {
        inner: [u8; 1000]
    }

    pub(super) fn test_allocate_at_simple<A: AllocatorModule, F: Fn(&mut A, &mut A, isize)>(
        mut heap1: A,
        mut heap2: A,
        check_heap_integrity: F,
    ) {
        let mut buffer1 = Buffer { inner: [0; 1000] };
        let mut buffer2 = Buffer { inner: [0; 1000] };
        let diff = (((&buffer2.inner[0]) as *const u8) as isize) - ((&buffer1.inner[0]) as *const u8) as isize;

        unsafe {
            heap1.init(&mut buffer1.inner[0], 2000 / 2);
            heap2.init(&mut buffer2.inner[0], 2000 / 2);

            let ptr1 = heap1.allocate(Layout::new::<u128>()).unwrap();
            let ptr2 = heap1.allocate(Layout::new::<u64>()).unwrap();
            let ptr3 = heap1.allocate(Layout::new::<u128>()).unwrap();

            heap1.deallocate(ptr1, Layout::new::<u128>());
            heap2
                .allocate_at(Layout::new::<u64>(), convert_ptr(ptr2, diff))
                .unwrap();
            heap2
                .allocate_at(Layout::new::<u128>(), convert_ptr(ptr3, diff))
                .unwrap();

            check_heap_integrity(&mut heap1, &mut heap2, diff);

            heap1.deallocate(ptr3, Layout::new::<u128>());
            heap2.deallocate(
                NonNull::new(convert_ptr(ptr3, diff)).unwrap(),
                Layout::new::<u128>(),
            );

            check_heap_integrity(&mut heap1, &mut heap2, diff);
        }
    }

    pub(super) fn test_allocate_at_restore_state<A: AllocatorModule, F: Fn(&mut A, &mut A, isize)>(
        mut heap1: A,
        mut heap2: A,
        check_heap_integrity: F,
    ) {
        let mut buffer1 = Buffer { inner: [0; 1000] };
        let mut buffer2 = Buffer { inner: [0; 1000] };
        let diff = (((&buffer2.inner[0]) as *const u8) as isize) - ((&buffer1.inner[0]) as *const u8) as isize;

        macro_rules! allocate {
            ($layout: expr) => {{
                let ptr1 = heap1.allocate($layout).unwrap();
                let ptr2 = heap2.allocate($layout).unwrap();
                (ptr1, ptr2, $layout)
            }};
        }

        macro_rules! deallocate {
            ($ptrs: ident) => {
                heap1.deallocate($ptrs.0, $ptrs.2);
                heap2.deallocate($ptrs.1, $ptrs.2);
            };
        }

        unsafe {
            heap1.init(&mut buffer1.inner[0], 2000 / 2);
            heap2.init(&mut buffer2.inner[0], 2000 / 2);

            let ptr1 = allocate!(Layout::new::<u128>());
            let ptr2 = allocate!(Layout::new::<u64>());
            let ptr3 = allocate!(Layout::new::<u8>());
            let ptr4 = allocate!(Layout::new::<u8>());
            let ptr5 = allocate!(Layout::new::<u64>());
            let ptr6 = allocate!(Layout::new::<u128>());

            deallocate!(ptr2);
            deallocate!(ptr6);

            // reset heap2
            heap2.reset();
            buffer2.inner.fill(0);

            heap2.init(&mut buffer2.inner[0], 2000 / 2);

            for ptrs in [ptr1, ptr3, ptr4, ptr5] {
                heap2.allocate_at(ptrs.2, ptrs.1.as_ptr()).unwrap();
            }

            check_heap_integrity(&mut heap1, &mut heap2, diff);
        }
    }
}
