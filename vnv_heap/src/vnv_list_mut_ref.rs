use core::{cell::RefCell, ops::Deref};
use std::mem::size_of;

use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    },
    resident_object_manager::resident_object_metadata::ResidentObjectMetadata,
    vnv_heap::VNVHeapInner,
};

pub struct VNVListMutRef<
    'a,
    'b,
    'c,
    'd: 'a,
    T: Sized + Copy,
    const SIZE: usize,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
    data_ref: &'b mut [T; SIZE],
    meta_ref: &'c mut ResidentObjectMetadata,
}

impl<
        'a,
        'b,
        'c,
        'd: 'a,
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVListMutRef<'a, 'b, 'c, 'd, T, SIZE, A, N, M>
{
    pub(crate) unsafe fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
        data_ref: &'b mut [T; SIZE],
        meta_ref: &'c mut ResidentObjectMetadata,
    ) -> Self {
        VNVListMutRef {
            vnv_heap,
            data_ref,
            meta_ref,
        }
    }
}

impl<
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVListMutRef<'_, '_, '_, '_, T, SIZE, A, N, M>
{
    fn set(&mut self, index: usize, data: T) {
        let mut wrapper = unsafe { self.meta_ref.inner.partial_dirtiness_tracking_info.get_wrapper(self.meta_ref) };
        let offset = index * size_of::<T>();

        wrapper.set_range_dirty(offset, size_of::<T>());

        self.data_ref[index] = data;
    }

    fn get(&self, index: usize) -> T {
        self.data_ref[index]
    }
}

impl<
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > Deref for VNVListMutRef<'_, '_, '_, '_, T, SIZE, A, N, M>
{
    type Target = [T; SIZE];

    fn deref(&self) -> &Self::Target {
        &self.data_ref
    }
}

impl<
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > Drop for VNVListMutRef<'_, '_, '_, '_, T, SIZE, A, N, M>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
            .borrow_mut()
            .release_partial_mut::<[T; SIZE]>(self.meta_ref)
        }
    }
}
