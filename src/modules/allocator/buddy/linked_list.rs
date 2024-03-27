// file from: https://github.com/rcore-os/buddy_system_allocator
//! Provide the intrusive LinkedList

use core::marker::PhantomData;
use core::{fmt, ptr};

/// An intrusive linked list
///
/// A clean room implementation of the one used in CS140e 2018 Winter
///
/// Thanks Sergio Benitez for his excellent work,
/// See [CS140e](https://cs140e.sergio.bz/) for more information
#[derive(Copy, Clone)]
pub struct LinkedList {
    head: *mut usize,
}

unsafe impl Send for LinkedList {}

impl LinkedList {
    /// Create a new LinkedList
    pub const fn new() -> LinkedList {
        LinkedList {
            head: ptr::null_mut(),
        }
    }

    /// Updates the pointers of this list using the given `ptr_offset` in bytes.
    /// 
    /// **Note**: Be careful with `ptr_offset` as some value can break the linked list
    /// (for example: `ptr_offset` has to be a multiple of `size_of::<usize>()`)
    pub unsafe fn update_ptrs(&mut self, ptr_offset: isize) {
        let mut curr = (&mut self.head) as *mut *mut usize;
        while !(*curr).is_null() {
            *curr = (*curr as *mut u8).offset(ptr_offset) as *mut usize;
            curr = (*curr) as *mut *mut usize;
        }
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Push `item` to the front of the list
    pub unsafe fn push(&mut self, item: *mut usize) {
        *item = self.head as usize;
        self.head = item;
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut usize> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head;
                self.head = unsafe { *item as *mut usize };
                Some(item)
            }
        }
    }

    /// Return an iterator over the items in the list
    pub fn iter(&self) -> Iter {
        Iter {
            curr: self.head,
            list: PhantomData,
        }
    }

    /// Return an mutable iterator over the items in the list
    pub fn iter_mut(&mut self) -> IterMut {
        IterMut {
            prev: &mut self.head as *mut *mut usize as *mut usize,
            curr: self.head,
            list: PhantomData,
        }
    }
}

impl fmt::Debug for LinkedList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// An iterator over the linked list
pub struct Iter<'a> {
    curr: *mut usize,
    list: PhantomData<&'a LinkedList>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = *mut usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let item = self.curr;
            let next = unsafe { *item as *mut usize };
            self.curr = next;
            Some(item)
        }
    }
}

/// Represent a mutable node in `LinkedList`
pub struct ListNode {
    prev: *mut usize,
    curr: *mut usize,
}

impl ListNode {
    /// Remove the node from the list
    pub fn pop(self) -> *mut usize {
        // Skip the current one
        unsafe {
            *(self.prev) = *(self.curr);
        }
        self.curr
    }

    /// Returns the pointed address
    pub fn value(&self) -> *mut usize {
        self.curr
    }
}

/// A mutable iterator over the linked list
pub struct IterMut<'a> {
    list: PhantomData<&'a mut LinkedList>,
    prev: *mut usize,
    curr: *mut usize,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = ListNode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let res = ListNode {
                prev: self.prev,
                curr: self.curr,
            };
            self.prev = self.curr;
            self.curr = unsafe { *self.curr as *mut usize };
            Some(res)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::modules::allocator::buddy::linked_list::LinkedList;

    #[test]
    fn test_ptr_update() {
        const ARR_SIZE: usize = 100;
        const VIRTUAL_PAGE_SIZE: usize = ARR_SIZE * 2;
        let mut virtual_page = [0usize; VIRTUAL_PAGE_SIZE];
        let origin = &mut virtual_page[0..ARR_SIZE];

        // some random items to test
        let list_indexes = [0usize, 10, 11, 5, 50, 68, 2, 3];
        let mut list_indexes_rev = list_indexes.clone();
        list_indexes_rev.reverse();

        let mut list = LinkedList::new();
        unsafe {
            for i in list_indexes_rev {
                list.push(&mut origin[i]);
            }
        }

        // make sure original list works as expected
        for (ptr, i) in list.iter().zip(list_indexes) {
            assert_eq!(&mut origin[i] as *mut usize, ptr);
        }

        // copy data to dest part
        for i in 0..ARR_SIZE {
            virtual_page[ARR_SIZE + i] = virtual_page[i]; 
        }

        let ptr_offset = unsafe { ((&virtual_page[ARR_SIZE] as *const usize) as *const u8).offset_from((&virtual_page[0] as *const usize) as *const u8) };
        let dest = &mut virtual_page[ARR_SIZE..VIRTUAL_PAGE_SIZE];

        unsafe { list.update_ptrs(ptr_offset) };

        // test if update ptrs was successful
        for (ptr, i) in list.iter().zip(list_indexes) {
            assert_eq!(&mut dest[i] as *mut usize, ptr);
        }
    }
}