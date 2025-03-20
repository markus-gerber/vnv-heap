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

use core::{cell::RefCell, ops::Deref, mem::size_of};

use crate::{
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
    },
    resident_object_manager::resident_object_metadata::ResidentObjectMetadata,
    vnv_heap::VNVHeapInner,
};

pub struct VNVArrayMutRef<
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
    > VNVArrayMutRef<'a, 'b, 'c, 'd, T, SIZE, A, N, M>
{
    pub(crate) unsafe fn new(
        vnv_heap: &'a RefCell<VNVHeapInner<'d, A, N, M>>,
        data_ref: &'b mut [T; SIZE],
        meta_ref: &'c mut ResidentObjectMetadata,
    ) -> Self {
        VNVArrayMutRef {
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
    > VNVArrayMutRef<'_, '_, '_, '_, T, SIZE, A, N, M>
{
    pub fn set(&mut self, index: usize, data: T) -> Result<(), ()> {
        let mut vnv_heap = self.vnv_heap.borrow_mut();
        let offset = index * size_of::<T>();

        // try to make this range dirty
        vnv_heap.partial_mut_make_range_dirty(self.meta_ref, offset, size_of::<T>())?;

        self.data_ref[index] = data;
        Ok(())
    }

    pub fn get(&self, index: usize) -> T {
        self.data_ref[index]
    }
}

impl<
        T: Sized + Copy,
        const SIZE: usize,
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
    > Deref for VNVArrayMutRef<'_, '_, '_, '_, T, SIZE, A, N, M>
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
    > Drop for VNVArrayMutRef<'_, '_, '_, '_, T, SIZE, A, N, M>
{
    fn drop(&mut self) {
        unsafe {
            self.vnv_heap
            .borrow_mut()
            .release_partial_mut::<[T; SIZE]>(self.meta_ref)
        }
    }
}
