use core::{alloc::Layout, cell::RefCell, marker::PhantomData};
use std::ptr::null_mut;

use crate::{
    allocation_identifier::AllocationIdentifier, modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        persistent_storage::PersistentStorageModule,
    }, resident_object::ResidentObjectIdentifier, vnv_heap::VNVHeapInner, vnv_mut_ref::VNVMutRef, vnv_ref::VNVRef
};

pub struct VNVObject<
    'a,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    S: PersistentStorageModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<A, N, S>>,
    allocation_identifier: AllocationIdentifier<T>,
    phantom_data: PhantomData<T>,
    pub(crate) resident_id: ResidentObjectIdentifier,
}

impl<
        'a,
        T: Sized,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        S: PersistentStorageModule,
    > VNVObject<'a, T, A, N, S>
{
    pub(crate) fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<A, N, S>>,
        identifier: AllocationIdentifier<T>,
    ) -> Self {
        VNVObject {
            vnv_heap,
            allocation_identifier: identifier,
            phantom_data: PhantomData,
            resident_id: ResidentObjectIdentifier::none()
        }
    }

    pub fn get(&self) -> VNVRef<'_, '_, '_, T, A, N, S> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *const T = heap.get_ref(&self.allocation_identifier, &self.resident_id);
            let data_ref = ptr.as_ref().unwrap();
            VNVRef::new(self.vnv_heap, &self.allocation_identifier, data_ref)
        }
    }

    pub fn get_mut(&mut self) -> VNVMutRef<'_, '_, '_, T, A, N, S> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *mut T = heap.get_mut(&self.allocation_identifier, &self.resident_id);
            let data_ref = ptr.as_mut().unwrap();
            VNVMutRef::new(self.vnv_heap, &self.allocation_identifier, data_ref)
        }
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule> Drop
    for VNVObject<'_, T, A, N, S>
{
    fn drop(&mut self) {
        let layout = Layout::new::<T>();
        let mut obj = self.vnv_heap.borrow_mut();
        unsafe {
            obj.deallocate(&layout, &self.allocation_identifier);
        }
    }
}

#[cfg(test)]
pub(crate) mod test {
    use crate::{
        allocation_identifier::AllocationIdentifier,
        modules::{
            allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
            persistent_storage::PersistentStorageModule,
        },
    };

    use super::VNVObject;

    // just for testing purposes
    impl<
            T: Sized,
            A: AllocatorModule,
            N: NonResidentAllocatorModule,
            S: PersistentStorageModule,
        > VNVObject<'_, T, A, N, S>
    {
        pub(crate) fn get_allocation_identifier(&self) -> &AllocationIdentifier<T> {
            &self.allocation_identifier
        }
    }
}
