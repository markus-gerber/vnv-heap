use core::{cell::RefCell, ops::Deref};

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    },
    vnv_heap::VNVHeapInner, vnv_list::ListItemContainer,
};

pub struct VNVListRef<
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
    data_ref: &'c ListItemContainer<T>,
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
    > VNVListRef<'a, 'b, 'c, 'd, T, A, N, M>
{
    pub(crate) unsafe fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
        allocation_identifier: &'b AllocationIdentifier<ListItemContainer<T>>,
        data_ref: &'c ListItemContainer<T>,
    ) -> Self {
        VNVListRef {
            vnv_heap,
            allocation_identifier,
            data_ref,
        }
    }

}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Deref
    for VNVListRef<'_, '_, '_, '_, T, A, N, M>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data_ref.data
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Drop
    for VNVListRef<'_, '_, '_, '_, T, A, N, M>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
                .borrow_mut()
                .release_ref(self.allocation_identifier)
        }
    }
}
