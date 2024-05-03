use core::{cell::RefCell, ops::Deref};

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
    vnv_heap::VNVHeapInner,
};

pub struct VNVRef<
    'a,
    'b,
    'c,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    S: PersistentStorageModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<A, N, S>>,
    allocation_identifier: &'b AllocationIdentifier<T>,
    data_ref: &'c T,
}

impl<
        'a,
        'b,
        'c,
        T: Sized,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        S: PersistentStorageModule,
    > VNVRef<'a, 'b, 'c, T, A, N, S>
{
    pub(crate) unsafe fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<A, N, S>>,
        allocation_identifier: &'b AllocationIdentifier<T>,
        data_ref: &'c T,
    ) -> Self {
        VNVRef {
            vnv_heap,
            allocation_identifier,
            data_ref,
        }
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule> Deref
    for VNVRef<'_, '_, '_, T, A, N, S>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule> Drop
    for VNVRef<'_, '_, '_, T, A, N, S>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
                .borrow_mut()
                .release_ref(self.allocation_identifier, self.data_ref)
        }
    }
}
