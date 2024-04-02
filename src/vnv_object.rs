use std::{alloc::Layout, cell::RefCell, marker::PhantomData, rc::Rc};

use crate::{modules::{allocator::AllocatorModule, page_replacement::PageReplacementModule, page_storage::PageStorageModule}, vnv_heap::VNVHeapInner, vnv_meta_store::AllocationIdentifier, vnv_mut_ref::VNVMutRef, vnv_ref::VNVRef};

pub struct VNVObject<T, A: AllocatorModule + 'static, R: PageReplacementModule, S: PageStorageModule> {
    vnv_heap: Rc<RefCell<VNVHeapInner<A, R, S>>>,
    allocation_identifier: AllocationIdentifier<T, A>,
    phantom_data: PhantomData<T>
}

impl<T: Sized, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule> VNVObject<T, A, R, S> {
    pub(crate) fn new(vnv_heap: Rc<RefCell<VNVHeapInner<A, R, S>>>, identifier: AllocationIdentifier<T, A>) -> Self {
        VNVObject {
            vnv_heap,
            allocation_identifier: identifier,
            phantom_data: PhantomData
        }
    }

    pub fn get(&self) -> VNVRef<'_, '_, T, A, R, S> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *const T = heap.get_ref(&self.allocation_identifier);
            let data_ref = ptr.as_ref().unwrap();
            VNVRef::new(Rc::clone(&self.vnv_heap), &self.allocation_identifier, data_ref)
        }
    }

    pub fn get_mut<'a>(&mut self) -> VNVMutRef<'_, '_, T, A, R, S> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *mut T =  heap.get_mut(&self.allocation_identifier);
            let data_ref = ptr.as_mut().unwrap() ;
            VNVMutRef::new(Rc::clone(&self.vnv_heap), &self.allocation_identifier, data_ref)
        }
    }
}

impl<'a, T: Sized, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule> Drop for VNVObject<T, A, R, S> {
    fn drop(&mut self) {
        let layout = Layout::new::<T>();
        let mut obj = self.vnv_heap.borrow_mut();
        unsafe {
            obj.deallocate(&layout, &mut self.allocation_identifier);
        }
    }
}

#[cfg(test)]
pub(crate) mod test {
    use crate::{modules::{allocator::AllocatorModule, page_replacement::PageReplacementModule, page_storage::PageStorageModule}, vnv_meta_store::AllocationIdentifier};

    use super::VNVObject;

    /// just for testing purposes
    pub(crate) fn obj_to_allocation_identifier<T: Sized, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule>(obj: &VNVObject<T, A, R, S>) -> &AllocationIdentifier<T, A> {
        &obj.allocation_identifier
    }
}