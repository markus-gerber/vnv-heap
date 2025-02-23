use super::KeyValueStoreImpl;
use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule, persistent_storage::PersistentStorageModule,
    },
    VNVHeap,
};

pub(super) struct VNVHeapKeyValueStoreImplementation<
    'a,
    A: AllocatorModule + 'static,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
> {
    manager: VNVHeap<'a, A, N, M, S>,
}

impl<
        'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule + 'static,
    > VNVHeapKeyValueStoreImplementation<'a, A, N, M, S>
{
    pub(super) fn new(heap: VNVHeap<'a, A, N, M, S>) -> Self {
        Self { manager: heap }
    }
}

type InternalPointer = usize;

fn pointer_to_identifier<T>(ptr: usize) -> AllocationIdentifier<T> {
    AllocationIdentifier::<T>::from_offset(ptr)
}

impl<
        'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
    > KeyValueStoreImpl<InternalPointer> for VNVHeapKeyValueStoreImplementation<'a, A, N, M, S>
{
    fn allocate<T>(&self, data: T) -> Result<InternalPointer, ()> {
        let mut inner = self.manager.get_inner().borrow_mut();
        let identifier = unsafe { inner.allocate(data, false)? };

        debug_assert!(inner.is_resident(&identifier));
        Ok(identifier.offset)
    }

    fn deallocate<T>(&self, ptr: &InternalPointer) {
        let mut inner = self.manager.get_inner().borrow_mut();
        let identifier = pointer_to_identifier::<T>(*ptr);
        debug_assert!(inner.is_resident(&identifier));

        unsafe {
            inner.deallocate(&identifier, false).unwrap();
        }
    }

    fn get<T: Copy>(&mut self, ptr: &InternalPointer) -> Result<T, ()> {
        let mut inner = self.manager.get_inner().borrow_mut();
        let identifier = pointer_to_identifier::<T>(*ptr);
        debug_assert!(inner.is_resident(&identifier));

        unsafe {
            let data = inner.get_ref(&identifier, false)?;
            let copy = data.as_ref().unwrap().clone();
            inner.release_ref(&identifier);
            Ok(copy)
        }
    }

    fn update<T>(&mut self, ptr: &InternalPointer, data: T) -> Result<(), ()> {
        let mut inner = self.manager.get_inner().borrow_mut();
        let identifier = pointer_to_identifier::<T>(*ptr);
        debug_assert!(inner.is_resident(&identifier));

        unsafe {
            let data_ptr = inner.get_mut(&identifier, false)?;
            *data_ptr = data;
            inner.release_mut(&identifier);
        }

        Ok(())
    }
    
    fn flush<T>(&mut self, ptr: &InternalPointer) -> Result<(), ()> {
        let mut inner = self.manager.get_inner().borrow_mut();
        let identifier = pointer_to_identifier::<T>(*ptr);

        inner.flush_object::<T>(&identifier)
    }
}
