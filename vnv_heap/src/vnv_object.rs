use core::{alloc::Layout, cell::RefCell, marker::PhantomData};

use crate::{
    allocation_identifier::AllocationIdentifier, modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        persistent_storage::PersistentStorageModule,
    }, vnv_heap::VNVHeapInner, vnv_mut_ref::VNVMutRef, vnv_ref::VNVRef
};

pub struct VNVObject<
    'a,
    'b: 'a,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    S: PersistentStorageModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, S>>,
    allocation_identifier: AllocationIdentifier<T>,
    phantom_data: PhantomData<T>,
}

impl<
        'a,
        'b: 'a,
        T: Sized,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        S: PersistentStorageModule,
    > VNVObject<'a, 'b, T, A, N, S>
{
    pub(crate) fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, S>>,
        identifier: AllocationIdentifier<T>,
    ) -> Self {
        VNVObject {
            vnv_heap,
            allocation_identifier: identifier,
            phantom_data: PhantomData
        }
    }

    pub fn get(&self) -> Result<VNVRef<'a, '_, '_, 'b, T, A, N, S>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *const T = heap.get_ref(&self.allocation_identifier)?;
            let data_ref = ptr.as_ref().unwrap();
            Ok(VNVRef::new(self.vnv_heap, &self.allocation_identifier, data_ref))
        }
    }

    pub fn get_mut(&mut self) -> Result<VNVMutRef<'a, '_, '_, 'b, T, A, N, S>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *mut T = heap.get_mut(&self.allocation_identifier)?;
            let data_ref = ptr.as_mut().unwrap();
            Ok(VNVMutRef::new(self.vnv_heap, &self.allocation_identifier, data_ref))
        }
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule> Drop
    for VNVObject<'_, '_, T, A, N, S>
{
    fn drop(&mut self) {
        let layout = Layout::new::<T>();
        let mut obj = self.vnv_heap.borrow_mut();
        unsafe {
            // TODO handle this error somehow?
            obj.deallocate(layout, &self.allocation_identifier).unwrap();
        }
    }
}
