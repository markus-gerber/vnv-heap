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

mod internal;
mod linked_list;

use core::{alloc::Layout, ptr::NonNull};

use super::AllocatorModule;
use internal::Heap;

/// Buddy allocator module
pub struct BuddyAllocatorModule<const ORDER: usize> {
    inner: Heap<ORDER>,
}

impl<const ORDER: usize> AllocatorModule for BuddyAllocatorModule<ORDER> {
    unsafe fn init(&mut self, start: *mut u8, size: usize) {
        self.inner.init(start as usize, size)
    }

    unsafe fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
        self.inner.alloc(layout)
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        self.inner.dealloc(ptr, layout)
    }
    
    unsafe fn reset(&mut self) {
        self.inner = Heap::new();
    }
    
    unsafe fn allocate_at(&mut self, layout: Layout, ptr: *mut u8) -> Result<(), ()> {
        self.inner.alloc_at(layout, ptr)
    }

    #[cfg(debug_assertions)]
    #[allow(unused)]
    fn debug(&mut self) {
        todo!();
    }

    #[cfg(debug_assertions)]
    fn dump(&mut self) -> String {
        self.inner.dump()
    }
}

impl<const ORDER: usize> BuddyAllocatorModule<ORDER> {
    pub fn new() -> Self {
        Self {
            inner: Heap::new()
        }
    }
}

#[cfg(test)]
mod test {
    use super::{super::test::*, internal, BuddyAllocatorModule};

    #[test]
    fn test_allocate_at_simple_buddy() {
        test_allocate_at_simple(BuddyAllocatorModule::<16>::new(), BuddyAllocatorModule::<16>::new(), |heap1, heap2, diff| {
            internal::test::check_heap_integrity(&mut heap1.inner, &mut heap2.inner, diff)
        })
    }

    #[test]
    fn test_allocate_at_restore_state_buddy() {
        test_allocate_at_restore_state(BuddyAllocatorModule::<16>::new(), BuddyAllocatorModule::<16>::new(), |heap1, heap2, diff| {
            internal::test::check_heap_integrity(&mut heap1.inner, &mut heap2.inner, diff)
        })
    }
}