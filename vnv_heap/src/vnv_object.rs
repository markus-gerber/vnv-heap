use core::{alloc::Layout, cell::RefCell, marker::PhantomData};

use crate::{
    modules::{
        allocator::AllocatorModule, memory_provider::MemoryProviderModule,
        page_replacement::PageReplacementModule, page_storage::PageStorageModule,
    },
    vnv_heap::VNVHeapInner,
    vnv_meta_store::AllocationIdentifier,
    vnv_mut_ref::VNVMutRef,
    vnv_ref::VNVRef,
};

pub struct VNVObject<
    'a,
    T,
    A: AllocatorModule + 'static,
    R: PageReplacementModule,
    S: PageStorageModule,
    M: MemoryProviderModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<A, R, S, M>>,
    allocation_identifier: AllocationIdentifier<T, A>,
    phantom_data: PhantomData<T>,
}

impl<
        'a,
        T: Sized,
        A: AllocatorModule,
        R: PageReplacementModule,
        S: PageStorageModule,
        M: MemoryProviderModule,
    > VNVObject<'a, T, A, R, S, M>
{
    pub(crate) fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<A, R, S, M>>,
        identifier: AllocationIdentifier<T, A>,
    ) -> Self {
        VNVObject {
            vnv_heap,
            allocation_identifier: identifier,
            phantom_data: PhantomData,
        }
    }

    pub fn get(&self) -> VNVRef<'_, '_, '_, T, A, R, S, M> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *const T = heap.get_ref(&self.allocation_identifier);
            let data_ref = ptr.as_ref().unwrap();
            VNVRef::new(
                self.vnv_heap,
                &self.allocation_identifier,
                data_ref,
            )
        }
    }

    pub fn get_mut(&mut self) -> VNVMutRef<'_, '_, '_, T, A, R, S, M> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *mut T = heap.get_mut(&self.allocation_identifier);
            let data_ref = ptr.as_mut().unwrap();
            VNVMutRef::new(
                self.vnv_heap,
                &self.allocation_identifier,
                data_ref,
            )
        }
    }
}

impl<
        T: Sized,
        A: AllocatorModule,
        R: PageReplacementModule,
        S: PageStorageModule,
        M: MemoryProviderModule,
    > Drop for VNVObject<'_, T, A, R, S, M>
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
        modules::{
            allocator::AllocatorModule, memory_provider::MemoryProviderModule,
            page_replacement::PageReplacementModule, page_storage::PageStorageModule,
        },
        vnv_meta_store::AllocationIdentifier,
    };

    use super::VNVObject;

    // just for testing purposes
    impl<T, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule, M: MemoryProviderModule> VNVObject<'_, T, A, R, S, M> {
        pub(crate) fn get_allocation_identifier(&self) -> &AllocationIdentifier<T, A> {
            &self.allocation_identifier
        }
    }
}
