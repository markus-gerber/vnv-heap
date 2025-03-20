/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use core::{cell::RefCell, marker::PhantomData};

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    },
    vnv_heap::VNVHeapInner,
    vnv_array_mut_ref::VNVArrayMutRef,
    vnv_mut_ref::VNVMutRef,
    vnv_ref::VNVRef,
};

pub struct VNVArray<
    'a,
    'b: 'a,
    T: Sized + Copy,
    const SIZE: usize,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, M>>,
    allocation_identifier: AllocationIdentifier<[T; SIZE]>,
    phantom_data: PhantomData<T>,
}

impl<
        'a,
        'b: 'a,
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > VNVArray<'a, 'b, T, SIZE, A, N, M>
{
    pub(crate) fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'b, A, N, M>>,
        identifier: AllocationIdentifier<[T; SIZE]>,
    ) -> Self {
        VNVArray {
            vnv_heap,
            allocation_identifier: identifier,
            phantom_data: PhantomData,
        }
    }

    pub fn get(&mut self) -> Result<VNVRef<'a, '_, '_, 'b, [T; SIZE], A, N, M>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *const [T; SIZE] = heap.get_ref(&self.allocation_identifier, true)?;
            let data_ref = ptr.as_ref().unwrap();
            Ok(VNVRef::new(
                self.vnv_heap,
                &self.allocation_identifier,
                data_ref,
            ))
        }
    }

    pub fn get_mut(&mut self) -> Result<VNVArrayMutRef<'a, '_, '_, 'b, T, SIZE, A, N, M>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        let (meta_ptr, data_ptr) =
            unsafe { heap.get_partial_mut::<[T; SIZE]>(&self.allocation_identifier)? };

        let meta_ref = unsafe { meta_ptr.as_mut().unwrap() };
        let data_ref = unsafe { data_ptr.as_mut().unwrap() };
        Ok(unsafe { VNVArrayMutRef::new(self.vnv_heap, data_ref, meta_ref) })
    }

    pub fn get_mut_whole_arr(
        &mut self,
    ) -> Result<VNVMutRef<'a, '_, '_, 'b, [T; SIZE], A, N, M>, ()> {
        let mut heap = self.vnv_heap.borrow_mut();
        unsafe {
            let ptr: *mut [T; SIZE] = heap.get_mut(&self.allocation_identifier, false)?;
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
        heap.unload_object(&self.allocation_identifier, true)
    }

    #[allow(unused)]
    pub(crate) fn get_alloc_id(&self) -> &AllocationIdentifier<[T; SIZE]> {
        return &self.allocation_identifier;
    }
}

impl<
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > Drop for VNVArray<'_, '_, T, SIZE, A, N, M>
{
    fn drop(&mut self) {
        let mut obj = self.vnv_heap.borrow_mut();
        unsafe {
            // TODO handle this error somehow?
            match obj.deallocate(&self.allocation_identifier, true) {
                Ok(()) => {}
                Err(()) => {
                    println!("could not deallocate");
                }
            }
        }
    }
}
