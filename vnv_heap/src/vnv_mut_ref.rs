use core::{
    cell::RefCell,
    ops::{Deref, DerefMut},
};

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    },
    vnv_heap::VNVHeapInner,
};

pub struct VNVMutRef<
    'a,
    'b,
    'c,
    'd: 'a,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
    allocation_identifier: &'b AllocationIdentifier<T>,
    data_ref: &'c mut T,
}

impl<
        'a,
        'b,
        'c,
        'd: 'a,
        T: Sized,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVMutRef<'a, 'b, 'c, 'd, T, A, N, M>
{
    pub(crate) unsafe fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
        allocation_identifier: &'b AllocationIdentifier<T>,
        data_ref: &'c mut T,
    ) -> Self {
        VNVMutRef {
            vnv_heap,
            allocation_identifier,
            data_ref,
        }
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Deref
    for VNVMutRef<'_, '_, '_, '_, T, A, N, M>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule>
    DerefMut for VNVMutRef<'_, '_, '_, '_, T, A, N, M>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_ref
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Drop
    for VNVMutRef<'_, '_, '_, '_, T, A, N, M>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
                .borrow_mut()
                .release_mut(self.allocation_identifier, self.data_ref)
        }
    }
}
