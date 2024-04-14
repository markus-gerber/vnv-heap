use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use crate::{
    modules::{
        allocator::AllocatorModule, memory_provider::MemoryProviderModule,
        page_replacement::PageReplacementModule, page_storage::PageStorageModule,
    },
    vnv_heap::VNVHeapInner,
    vnv_meta_store::AllocationIdentifier,
};

pub struct VNVMutRef<
    'a,
    'b,
    T: Sized,
    A: AllocatorModule + 'static,
    R: PageReplacementModule,
    P: PageStorageModule,
    M: MemoryProviderModule,
> {
    vnv_heap: Rc<RefCell<VNVHeapInner<A, R, P, M>>>,
    allocation_identifier: &'b AllocationIdentifier<T, A>,
    data_ref: &'a mut T,
}

impl<
        'a,
        'b,
        T: Sized,
        A: AllocatorModule,
        R: PageReplacementModule,
        P: PageStorageModule,
        M: MemoryProviderModule,
    > VNVMutRef<'a, 'b, T, A, R, P, M>
{
    pub(crate) unsafe fn new(
        vnv_heap: Rc<RefCell<VNVHeapInner<A, R, P, M>>>,
        allocation_identifier: &'b AllocationIdentifier<T, A>,
        data_ref: &'a mut T,
    ) -> Self {
        VNVMutRef {
            vnv_heap,
            allocation_identifier,
            data_ref,
        }
    }
}

impl<
        T: Sized,
        A: AllocatorModule,
        R: PageReplacementModule,
        P: PageStorageModule,
        M: MemoryProviderModule,
    > Deref for VNVMutRef<'_, '_, T, A, R, P, M>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<
        T: Sized,
        A: AllocatorModule,
        R: PageReplacementModule,
        P: PageStorageModule,
        M: MemoryProviderModule,
    > DerefMut for VNVMutRef<'_, '_, T, A, R, P, M>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_ref
    }
}

impl<
        T: Sized,
        A: AllocatorModule,
        R: PageReplacementModule,
        P: PageStorageModule,
        M: MemoryProviderModule,
    > Drop for VNVMutRef<'_, '_, T, A, R, P, M>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
                .borrow_mut()
                .release_mut(self.allocation_identifier, self.data_ref)
        }
    }
}
