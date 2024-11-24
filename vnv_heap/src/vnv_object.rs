use core::{cell::RefCell, marker::PhantomData};

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    },
    vnv_heap::VNVHeapInner,
    vnv_mut_ref::VNVMutRef,
    vnv_ref::VNVRef,
};

pub struct VNVObject<
    'a,
    'b: 'a,
    T: Sized,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, M>>,
    allocation_identifier: AllocationIdentifier<T>,
    phantom_data: PhantomData<T>,
}

impl<
        'a,
        'b: 'a,
        T: Sized,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVObject<'a, 'b, T, A, N, M>
{
    pub(crate) fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, M>>,
        identifier: AllocationIdentifier<T>,
    ) -> Self {
        VNVObject {
            vnv_heap,
            allocation_identifier: identifier,
            phantom_data: PhantomData,
        }
    }

    pub fn get(&mut self) -> Result<VNVRef<'a, '_, '_, 'b, T, A, N, M>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *const T = heap.get_ref(&self.allocation_identifier)?;
            let data_ref = ptr.as_ref().unwrap();
            Ok(VNVRef::new(
                self.vnv_heap,
                &self.allocation_identifier,
                data_ref,
            ))
        }
    }

    pub fn get_mut(
        &mut self,
    ) -> Result<VNVMutRef<'a, '_, '_, 'b, T, A, N, M>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *mut T = heap.get_mut(&self.allocation_identifier)?;
            let data_ref = ptr.as_mut().unwrap();
            Ok(VNVMutRef::new(
                self.vnv_heap,
                &self.allocation_identifier,
                data_ref,
            ))
        }
    }

    pub fn is_resident(&self) -> bool {
        let mut heap = self.vnv_heap.borrow_mut();
        heap.is_resident(&self.allocation_identifier)
    }

    pub fn unload(&mut self) -> Result<(), ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        heap.unload_object(&self.allocation_identifier)
    }

    #[allow(unused)]
    pub(crate) fn get_alloc_id(&self) -> &AllocationIdentifier<T> {
        return &self.allocation_identifier;
    }
}

impl<T: Sized, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule> Drop
    for VNVObject<'_, '_, T, A, N, M>
{
    fn drop(&mut self) {
        let mut obj = self.vnv_heap.borrow_mut();
        unsafe {
            // TODO handle this error somehow?
            match obj.deallocate(&self.allocation_identifier) {
                Ok(()) => {}
                Err(()) => {
                    println!("could not deallocate");
                }
            }
        }
    }
}
