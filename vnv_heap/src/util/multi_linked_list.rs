use core::{marker::PhantomData, ptr::null_mut};

pub(crate) struct MultiLinkedList<T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> {
    head: *mut T,

    /// function to get the next element field of that item
    get_next_element_field: F,
    get_element_field: G,
}

impl<T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> MultiLinkedList<T, E, F, G> {
    /// ### Safety
    ///
    /// It is unsafe to create multiple `MultiLinkedList`s over the same next field
    /// (the same `get_next_element_field` function)
    pub(crate) unsafe fn new(get_next_element_field: F, get_element_field: G) -> Self {
        Self {
            head: null_mut(),
            get_next_element_field,
            get_element_field,
        }
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Push `item` to the front of the list
    pub fn push(&mut self, item: &mut T) {
        let next = (self.get_next_element_field)(item);
        unsafe { *next = self.head };

        self.head = item;
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut T> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head;
                self.head = unsafe { *(self.get_next_element_field)(item) };
                Some(item)
            }
        }
    }

    /// Returns an iterator over the items in the list
    pub fn iter(&self) -> Iter<'_, '_, '_, T, E, F, G> {
        let head = if !self.head.is_null() {
            Some(unsafe { self.head.as_ref().unwrap() })
        } else {
            None
        };

        Iter {
            curr: head,
            list: PhantomData,
            get_next_element: &self.get_next_element_field,
            get_element: &self.get_element_field,
        }
    }

    /// Returns a mutable iterator over the items in the list
    pub fn iter_mut(&mut self) -> IterMut<'_, '_, T, E, F, G> {
        IterMut {
            curr: CurrItem {
                curr: self.head,

                // don't advance prev for first item
                advance_prev: false,
                prev: &mut self.head,
                get_next: &self.get_next_element_field,
                get_element: &self.get_element_field,
                _phantom_data: PhantomData,
            },
        }
    }
}

/// An iterator over the linked list
pub struct Iter<'a, 'b, 'c, T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> {
    curr: Option<&'b T>,
    list: PhantomData<&'a MultiLinkedList<T, E, F, G>>,
    get_next_element: &'c F,
    get_element: &'c G,
}

impl<'a, 'b, 'c, T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> Iterator
    for Iter<'a, 'b, 'c, T, E, F, G>
{
    type Item = &'b T;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr.take();

        if let Some(data) = ret {
            // some ugly pointer magic (because func only takes *mut pointers)
            // this is safe, because we don't access the pointers here
            let ptr = (self.get_next_element)((data as *const T) as *mut T) as *const *const T;
            let next_ptr = unsafe { *ptr };
            if !next_ptr.is_null() {
                self.curr = Some(unsafe { next_ptr.as_ref().unwrap() })
            }
        }

        ret
    }
}

pub struct DeleteHandle<'a, 'b, 'c, T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> {
    inner: &'c mut CurrItem<'a, 'b, T, E, F, G>,
}

impl<T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E>
    DeleteHandle<'_, '_, '_, T, E, F, G>
{
    /// Delete an item
    #[inline]
    pub fn delete(self) {
        let new_ptr = (self.inner.get_next)(self.inner.curr);
        unsafe { *self.inner.prev = *new_ptr };
        self.inner.advance_prev = false;
    }

    #[inline]
    pub fn get_container_ptr(&mut self) -> *mut T {
        self.inner.curr
    }

    /// ### Safety
    ///
    /// Make sure that you don't have any open references to any objects of this list.
    #[inline]
    pub unsafe fn get_element(&mut self) -> &mut E {
        (self.inner.get_element)(self.inner.curr).as_mut().unwrap()
    }
}

struct CurrItem<'a, 'b, T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> {
    prev: *mut *mut T,
    curr: *mut T,
    advance_prev: bool,
    get_next: &'b F,
    get_element: &'b G,
    _phantom_data: PhantomData<&'a ()>,
}

pub struct IterMut<'a, 'b, T, E, F: Fn(*mut T) -> *mut *mut T, G: Fn(*mut T) -> *mut E> {
    curr: CurrItem<'a, 'b, T, E, F, G>,
}

impl<'a, 'b, T, E, F: 'a + Fn(*mut T) -> *mut *mut T, G: 'a + Fn(*mut T) -> *mut E>
    IterMut<'a, 'b, T, E, F, G>
{
    pub fn next<'c>(&'c mut self) -> Option<DeleteHandle<'a, 'b, 'c, T, E, F, G>> {
        if self.curr.curr == null_mut() {
            None
        } else {
            // if the previous element was deleted we don't want to update
            // the previous/drag pointer (as previous item is the same for the next item)
            if self.curr.advance_prev {
                // advance curr item and drag pointer
                let new_prev = (self.curr.get_next)(self.curr.curr);

                self.curr.prev = unsafe { new_prev.as_mut().unwrap() };
            }
            self.curr.advance_prev = true;
            self.curr.curr = unsafe { *self.curr.prev };

            if self.curr.curr == null_mut() {
                None
            } else {
                Some(DeleteHandle {
                    inner: &mut self.curr,
                })
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::VecDeque, fmt::Debug, ptr::null_mut};

    use memoffset::offset_of;

    use super::MultiLinkedList;

    #[derive(Clone, Copy)]
    struct TestStructData {
        a: usize,
        b: bool,
    }

    impl Debug for TestStructData {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("TestStruct")
            .field("a", &self.a)
            .field("b", &self.b)
                .finish()
        }
    }

    impl PartialEq for TestStructData {
        fn eq(&self, other: &Self) -> bool {
            self.a == other.a && self.b == other.b
        }
    }

    #[derive(Clone, Copy)]
    struct TestStruct {
        next_x: *mut TestStruct,
        next_y: *mut TestStruct,
        data: TestStructData,
    }

    impl TestStruct {
        fn new(a: usize, b: bool) -> Self {
            Self {
                next_x: null_mut(),
                next_y: null_mut(),
                data: TestStructData { a, b },
            }
        }

        fn get_next_x(ptr: *mut TestStruct) -> *mut *mut TestStruct {
            const OFFSET: usize = offset_of!(TestStruct, next_x);
            (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut *mut TestStruct
        }

        fn get_next_y(ptr: *mut TestStruct) -> *mut *mut TestStruct {
            const OFFSET: usize = offset_of!(TestStruct, next_y);
            (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut *mut TestStruct
        }

        fn get_data(ptr: *mut TestStruct) -> *mut TestStructData {
            const OFFSET: usize = offset_of!(TestStruct, data);
            (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut TestStructData
        }
    }

    impl Debug for TestStruct {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("TestStruct")
                .field("data", &self.data)
                .finish()
        }
    }

    impl PartialEq for TestStruct {
        fn eq(&self, other: &Self) -> bool {
            self.data == other.data
        }
    }

    struct TestableModularLinkedList<
        T: Clone + Copy + PartialEq + Debug,
        E: Clone + Copy + PartialEq + Debug,
        F: Fn(*mut T) -> *mut *mut T,
        G: Fn(*mut T) -> *mut E,
    > {
        check_list: VecDeque<T>,
        list: MultiLinkedList<T, E, F, G>,
    }

    impl<
            T: Clone + Copy + PartialEq + Debug,
            E: Clone + Copy + PartialEq + Debug,
            F: Fn(*mut T) -> *mut *mut T,
            G: Fn(*mut T) -> *mut E,
        > TestableModularLinkedList<T, E, F, G>
    {
        pub unsafe fn new(function: F, function2: G) -> Self {
            Self {
                check_list: VecDeque::new(),
                list: MultiLinkedList::new(function, function2),
            }
        }

        pub fn push(&mut self, item: &mut T) {
            self.check_list.push_front(item.clone());
            self.list.push(item);

            self.check_integrity();
        }

        pub fn pop(&mut self) -> Option<*mut T> {
            let item = self.check_list.pop_front();
            let item_2 = self.list.pop();

            assert_eq!(item.is_none(), item_2.is_none());
            if item.is_none() {
                return None;
            }

            let item = item.unwrap();
            let item_2 = item_2.unwrap();

            assert!(&item == unsafe { item_2.as_ref() }.unwrap());
            self.check_integrity();

            Some(item_2)
        }

        pub fn check_integrity(&self) {
            assert_eq!(unsafe { self.list.iter() }.count(), self.check_list.len());

            for (a, b) in unsafe { self.list.iter() }.zip(self.check_list.iter()) {
                println!("{:?} =?= {:?}", a, b);
                if a != b {
                    assert!(a == b);
                }
            }
        }

        pub fn print(&self) {
            print!("list: [");
            for x in unsafe { self.list.iter() } {
                print!("{:?}, ", x);
            }
            println!("]");

            print!("check_list: [");
            for x in self.check_list.iter() {
                print!("{:?}, ", x);
            }
            println!("]");
        }
    }

    #[test]
    fn test_simple() {
        let mut list = [
            TestStruct::new(12, false),
            TestStruct::new(1421, false),
            TestStruct::new(0, true),
            TestStruct::new(39, false),
            TestStruct::new(1, true),
            TestStruct::new(121983, true),
            TestStruct::new(11, true),
            TestStruct::new(24, false),
            TestStruct::new(1, true),
        ];

        let mut list_x =
            unsafe { TestableModularLinkedList::new(TestStruct::get_next_x, TestStruct::get_data) };
        let mut list_y =
            unsafe { TestableModularLinkedList::new(TestStruct::get_next_y, TestStruct::get_data) };

        macro_rules! check_integrity {
            () => {{
                list_x.check_integrity();
                list_y.check_integrity();
            }};
        }

        list_x.push(&mut list[1]);
        check_integrity!();
        list_x.push(&mut list[5]);
        check_integrity!();
        list_x.push(&mut list[4]);
        check_integrity!();
        list_x.push(&mut list[3]);
        check_integrity!();
        list_x.push(&mut list[2]);
        check_integrity!();
        list_x.push(&mut list[8]);
        check_integrity!();

        list_x.pop();
        check_integrity!();
        list_x.pop();
        check_integrity!();
        list_x.pop();
        check_integrity!();

        list_x.push(&mut list[3]);
        check_integrity!();
        list_x.push(&mut list[2]);
        check_integrity!();
        list_x.push(&mut list[8]);
        check_integrity!();

        list_y.push(&mut list[2]);
        check_integrity!();
        list_y.push(&mut list[5]);
        check_integrity!();
        list_y.push(&mut list[4]);
        check_integrity!();
        list_y.push(&mut list[0]);
        check_integrity!();
        list_y.push(&mut list[1]);
        check_integrity!();
        list_y.push(&mut list[8]);
        check_integrity!();

        list_y.pop();
        check_integrity!();
        list_y.pop();
        check_integrity!();
        list_y.pop();
        check_integrity!();

        list_y.push(&mut list[3]);
        check_integrity!();
        list_y.push(&mut list[1]);
        check_integrity!();
        list_y.push(&mut list[8]);
        check_integrity!();

        {
            println!("### before iter_mut ###");
            list_x.print();
            println!("### iter_mut ###");
            let mut iter = list_x.list.iter_mut();
            while let Some(mut handle) = iter.next() {
                let item = unsafe { handle.get_element() };
                let stop = item.a == 0;

                if item.b {
                    println!("delete {:?}", item);
                    handle.delete();
                }

                if stop {
                    break;
                }
            }

            list_x.check_list = list_x
                .check_list
                .into_iter()
                .fold(
                    (VecDeque::new(), false),
                    |mut acc: (VecDeque<TestStruct>, bool), item| {
                        if acc.1 {
                            acc.0.push_back(item);
                            acc
                        } else {
                            if !item.data.b {
                                acc.0.push_back(item)
                            }
                            if item.data.a == 0 {
                                acc.1 = true;
                            }
                            acc
                        }
                    },
                )
                .0;

            list_x.print();
        }
        check_integrity!();

        {
            println!("### before iter_mut ###");
            list_x.print();
            println!("### iter_mut ###");
            let mut iter = list_x.list.iter_mut();
            while let Some(handle) = iter.next() {
                handle.delete();
                break;
            }
            list_x.check_list.pop_front();
            list_x.print();
        }
        check_integrity!();

        {
            println!("### before iter_mut ###");
            list_y.print();
            println!("### iter_mut ###");
            let mut iter = list_y.list.iter_mut();
            while let Some(mut handle) = iter.next() {
                if unsafe { handle.get_element().a } == 0 {
                    handle.delete();
                }
            }
            list_y.check_list.pop_back();
            list_y.print();
        }
        check_integrity!();

        {
            println!("### before iter_mut ###");
            list_y.print();
            println!("### iter_mut ###");
            let mut iter = list_y.list.iter_mut();
            while let Some(handle) = iter.next() {
                handle.delete();
            }
            list_y.check_list.clear();
            list_y.print();
        }
        check_integrity!();
    
    }

    #[test]
    fn test_simple_filled() {
        let mut list = [
            TestStruct::new(12, false),
            TestStruct::new(1421, false),
            TestStruct::new(1, true),
            TestStruct::new(39, false),
            TestStruct::new(0, true),
            TestStruct::new(121983, true),
            TestStruct::new(11, true),
            TestStruct::new(24, false),
            TestStruct::new(1, true),
        ];

        let mut list_x =
            unsafe { TestableModularLinkedList::new(TestStruct::get_next_x, TestStruct::get_data) };
        let mut list_y =
            unsafe { TestableModularLinkedList::new(TestStruct::get_next_y, TestStruct::get_data) };

        macro_rules! check_integrity {
            () => {{
                list_x.check_integrity();
                list_y.check_integrity();
            }};
        }

        list_x.push(&mut list[7]);
        check_integrity!();
        list_x.push(&mut list[6]);
        check_integrity!();
        list_x.push(&mut list[3]);
        check_integrity!();
        list_x.push(&mut list[2]);
        check_integrity!();
        list_x.push(&mut list[5]);
        check_integrity!();
        list_x.push(&mut list[4]);
        check_integrity!();
        list_x.push(&mut list[8]);
        check_integrity!();
        list_x.push(&mut list[1]);
        check_integrity!();
        list_x.push(&mut list[0]);
        check_integrity!();

        list_y.push(&mut list[0]);
        check_integrity!();
        list_y.push(&mut list[7]);
        check_integrity!();
        list_y.push(&mut list[8]);
        check_integrity!();
        list_y.push(&mut list[2]);
        check_integrity!();
        list_y.push(&mut list[3]);
        check_integrity!();
        list_y.push(&mut list[1]);
        check_integrity!();
        list_y.push(&mut list[6]);
        check_integrity!();
        list_y.push(&mut list[5]);
        check_integrity!();
        list_y.push(&mut list[4]);
        check_integrity!();

        for _ in 0..9 {
            list_x.pop();
            check_integrity!();

            list_y.pop();
            check_integrity!();
        }
    }
}
