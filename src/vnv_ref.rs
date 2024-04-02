use std::{cell::RefCell, ops::Deref, rc::Rc};

use crate::{modules::{allocator::AllocatorModule, page_replacement::PageReplacementModule, page_storage::PageStorageModule}, vnv_heap::VNVHeapInner, vnv_meta_store::AllocationIdentifier};


pub struct VNVRef<'a, 'b, T: Sized, A: AllocatorModule + 'static, R: PageReplacementModule, P: PageStorageModule> {
    vnv_heap: Rc<RefCell<VNVHeapInner<A, R, P>>>,
    allocation_identifier: &'b AllocationIdentifier<T, A>,
    data_ref: &'a T
}

impl<'a, 'b, 'c, T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> VNVRef<'a, 'b, T, A, R, P> {
    pub(crate) unsafe fn new(vnv_heap: Rc<RefCell<VNVHeapInner<A, R, P>>>, allocation_identifier: &'b AllocationIdentifier<T, A>, data_ref: &'a T) -> Self {
        VNVRef {
            vnv_heap,
            allocation_identifier,
            data_ref
        }
    }
}

impl<T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> Deref for VNVRef<'_, '_, T, A, R, P> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> Drop for VNVRef<'_, '_, T, A, R, P> {
    fn drop(&mut self) {
        unsafe { self.vnv_heap.borrow_mut().release_ref(self.allocation_identifier, self.data_ref) }
    }
}