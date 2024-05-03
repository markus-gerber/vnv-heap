use std::{marker::PhantomData, ptr::null_mut};

pub(crate) struct ModularLinkedList<T, F: Fn(&mut T) -> &mut *mut T> {
    head: *mut T,

    /// function to get the next element field of that item
    get_next_element_field: F,
}

impl<T, F: Fn(&mut T) -> &mut *mut T> ModularLinkedList<T, F> {
    pub(crate) fn new(get_next_element_field: F) -> Self {
        Self {
            head: null_mut(),
            get_next_element_field,
        }
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Push `item` to the front of the list
    pub unsafe fn push(&mut self, item: &mut T) {
        let next = (self.get_next_element_field)(item);
        *next = self.head;

        self.head = item;
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut T> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head;
                self.head = *(self.get_next_element_field)(unsafe { item.as_mut().unwrap() });
                Some(item)
            }
        }
    }

    /// Return an iterator over the items in the list
    pub fn iter(&self) -> Iter<'_, '_, '_, T, F> {
        let head = if !self.head.is_null() {
            unsafe { Some(self.head.as_mut().unwrap()) }
        } else {
            None
        };

        Iter {
            curr: head,
            list: PhantomData,
            func: &self.get_next_element_field,
        }
    }
}

/// An iterator over the linked list
pub struct Iter<'a, 'b, 'c, T, F: Fn(&mut T) -> &mut *mut T> {
    curr: Option<&'b mut T>,
    list: PhantomData<&'a ModularLinkedList<T, F>>,
    func: &'c F,
}

impl<'a, 'b, 'c, T, F: Fn(&mut T) -> &mut *mut T> Iterator for Iter<'a, 'b, 'c, T, F> {
    type Item = &'b mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = self.curr.take();

        if let Some(data) = &mut ret {
            let item = *(self.func)(data);
            if !item.is_null() {
                self.curr = Some(unsafe { item.as_mut().unwrap() })
            }
        }

        ret
    }
}
