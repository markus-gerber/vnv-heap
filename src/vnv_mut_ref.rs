use std::{cell::RefCell, ops::{Deref, DerefMut}, rc::Rc};

use crate::{modules::{allocator::AllocatorModule, page_replacement::PageReplacementModule, page_storage::PageStorageModule}, vnv_heap::VNVHeapInner, vnv_meta_store::AllocationIdentifier};

pub struct VNVMutRef<'a, 'b, T: Sized, A: AllocatorModule + 'static, R: PageReplacementModule, P: PageStorageModule> {
    vnv_heap: Rc<RefCell<VNVHeapInner<A, R, P>>>,
    allocation_identifier: &'b AllocationIdentifier<T, A>,
    data_ref: &'a mut T
}

impl<'a, 'b, T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> VNVMutRef<'a, 'b, T, A, R, P> {
    pub(crate) unsafe fn new(vnv_heap: Rc<RefCell<VNVHeapInner<A, R, P>>>, allocation_identifier: &'b AllocationIdentifier<T, A>, data_ref: &'a mut T) -> Self {
        VNVMutRef {
            vnv_heap,
            allocation_identifier,
            data_ref
        }
    }
}

impl<T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> Deref for VNVMutRef<'_, '_, T, A, R, P> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> DerefMut for VNVMutRef<'_, '_, T, A, R, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_ref
    }
}

impl<T: Sized, A: AllocatorModule, R: PageReplacementModule, P: PageStorageModule> Drop for VNVMutRef<'_, '_, T, A, R, P> {
    fn drop(&mut self) {
        unsafe { self.vnv_heap.borrow_mut().release_mut(self.allocation_identifier, self.data_ref) }
    }
}