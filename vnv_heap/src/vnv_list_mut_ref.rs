use core::{cell::RefCell, ops::{Deref, DerefMut}};

use crate::{allocation_identifier::AllocationIdentifier, modules::{allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule, object_management::ObjectManagementModule}, vnv_list::ListItemContainer, VNVHeapInner};
pub struct VNVListMutRef<
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
    allocation_identifier: &'b AllocationIdentifier<ListItemContainer<T>>,
    data_ref: &'c mut ListItemContainer<T>,
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
    > VNVListMutRef<'a, 'b, 'c, 'd, T, A, N, M>
{
    pub(crate) unsafe fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
        allocation_identifier: &'b AllocationIdentifier<ListItemContainer<T>>,
        data_ref: &'c mut ListItemContainer<T>,
    ) -> Self {
        VNVListMutRef {
            vnv_heap,
            allocation_identifier,
            data_ref,
        }
    }

}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Deref
    for VNVListMutRef<'_, '_, '_, '_, T, A, N, M>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data_ref.data
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule>
    DerefMut for VNVListMutRef<'_, '_, '_, '_, T, A, N, M>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data_ref.data
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Drop
    for VNVListMutRef<'_, '_, '_, '_, T, A, N, M>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
                .borrow_mut()
                .release_mut(self.allocation_identifier)
        }
    }
}
