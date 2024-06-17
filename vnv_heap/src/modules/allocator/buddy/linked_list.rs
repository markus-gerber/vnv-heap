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

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Add `item` to the linked list (sorted)
    pub unsafe fn push(&mut self, item: *mut usize) {
        let mut prev: *mut *mut usize = &mut self.head;
        let mut curr: *mut usize = *prev;

        while !curr.is_null() {
            if (curr as usize) > (item as usize) {
                break;
            }

            // advance pointer
            prev = curr as *mut *mut usize;
            curr = *prev;
        }

        *item = curr as usize;
        *prev = item;
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
pub(super) mod test {
    use std::ptr::null_mut;

    use super::LinkedList;

    pub(crate) fn check_linked_list_integrity(list1: &mut LinkedList, list2: &mut LinkedList, diff: isize) {
        let mut iter1 = list1.iter();
        let mut iter2 = list2.iter();

        while let (Some(ptr1), Some(ptr2)) = (iter1.next(), iter2.next()) {
            assert_eq!(ptr1 as isize + diff, ptr2 as isize);
        }

        assert!(iter1.next().is_none());
        assert!(iter2.next().is_none());
    }

    #[test]
    fn test_linked_list_ordered() {
        fn check_ordered(list: &mut LinkedList) {
            let mut last_ptr = null_mut();
            for ptr in list.iter() {
                assert!((ptr as usize) >= last_ptr as usize);
                last_ptr = ptr;
            }
        }

        let mut buffer = [0usize; 200];
        let mut list = LinkedList::new();

        unsafe {
            let mut items = [2, 1, 0, 29, 199, 3, 10, 5, 12, 13, 15, 20, 14, 50];
            for x in items {
                list.push(&mut buffer[x]);
                check_ordered(&mut list);
            }

            items.sort();
            for x in items {
                assert_eq!(list.pop().unwrap() as usize, ((&mut buffer[x]) as *mut usize) as usize);        
            }

            assert!(list.pop().is_none());
        }
    }
}