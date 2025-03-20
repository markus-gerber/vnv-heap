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

mod hole;
mod internal;

use core::{alloc::Layout, ptr::NonNull};

use super::AllocatorModule;
use internal::Heap;

/// Linked list allocator module that uses first fit
pub struct LinkedListAllocatorModule {
    inner: Heap,
}

impl AllocatorModule for LinkedListAllocatorModule {
    unsafe fn init(&mut self, start: *mut u8, size: usize) {
        self.inner.init(start, size)
    }

    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
        self.inner.allocate_first_fit(layout)
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.inner.deallocate(ptr, layout)
    }

    unsafe fn reset(&mut self) {
        self.inner = Heap::empty()
    }
    
    unsafe fn allocate_at(&mut self, layout: Layout, ptr: *mut u8) -> Result<(), ()> {
        self.inner.allocate_at(layout, ptr)
    }

    #[cfg(debug_assertions)]
    #[allow(unused)]
    fn debug(&mut self) {
        self.inner.debug();
    }

    #[cfg(debug_assertions)]
    fn dump(&mut self) -> String {
        self.inner.dump()
    }
}

impl LinkedListAllocatorModule {
    pub fn new() -> Self {
        Self {
            inner: Heap::empty()
        }
    }
}


#[cfg(test)]
mod test {
    use crate::modules::allocator::LinkedListAllocatorModule;
    use super::{super::test::*, internal};

    #[test]
    fn test_allocate_at_simple_linked_list() {
        test_allocate_at_simple(LinkedListAllocatorModule::new(), LinkedListAllocatorModule::new(), |heap1, heap2, diff| {
            internal::test::check_heap_integrity(&mut heap1.inner, &mut heap2.inner, diff)
        })
    }
    
    #[test]
    fn test_allocate_at_restore_state_linked_list() {
        test_allocate_at_restore_state(LinkedListAllocatorModule::new(), LinkedListAllocatorModule::new(), |heap1, heap2, diff| {
            internal::test::check_heap_integrity(&mut heap1.inner, &mut heap2.inner, diff)
        })
    }
}