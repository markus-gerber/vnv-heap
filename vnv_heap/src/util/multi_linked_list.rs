use core::{
    cell::Cell,
    marker::PhantomData,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};

pub(crate) type DefaultMultiLinkedList<T, E, F, G> =
    GeneralMultiLinkedList<T, MultiLinkedListDefaultPointer<T>, E, F, G>;
pub(crate) type AtomicMultiLinkedList<T, E, F, G> =
    GeneralMultiLinkedList<T, MultiLinkedListAtomicPointer<T>, E, F, G>;

pub(crate) struct MultiLinkedListAtomicPointer<T> {
    inner: AtomicPtr<T>,
}

impl<T> MultiLinkedListPointer<T> for MultiLinkedListAtomicPointer<T> {
    #[inline]
    fn null() -> Self {
        Self {
            inner: AtomicPtr::new(null_mut()),
        }
    }

    #[inline]
    fn get(&self) -> *mut T {
        self.inner.load(Ordering::SeqCst)
    }

    #[inline]
    fn set(&self, ptr: *mut T) {
        self.inner.store(ptr, Ordering::SeqCst)
    }

    #[inline]
    fn is_null(&self) -> bool {
        self.get().is_null()
    }
}

pub(crate) struct MultiLinkedListDefaultPointer<T> {
    inner: Cell<*mut T>,
}

impl<T> MultiLinkedListPointer<T> for MultiLinkedListDefaultPointer<T> {
    #[inline]
    fn null() -> Self {
        Self {
            inner: Cell::new(null_mut()),
        }
    }

    #[inline]
    fn get(&self) -> *mut T {
        self.inner.get()
    }

    #[inline]
    fn set(&self, ptr: *mut T) {
        self.inner.set(ptr);
    }

    #[inline]
    fn is_null(&self) -> bool {
        self.inner.get().is_null()
    }
}

pub(crate) trait MultiLinkedListPointer<T> {
    fn null() -> Self;

    fn get(&self) -> *mut T;

    fn set(&self, ptr: *mut T);

    fn is_null(&self) -> bool;
}

pub(crate) struct GeneralMultiLinkedList<
    T,
    P: MultiLinkedListPointer<T>,
    E,
    F: Fn(*mut T) -> *mut P,
    G: Fn(*mut T) -> *mut E,
> {
    head: P,

    /// function to get the next element field of that item
    get_next_element_field: F,
    get_element_field: G,
    _phantom_data: PhantomData<T>,
}

impl<T, P: MultiLinkedListPointer<T>, E, F: Fn(*mut T) -> *mut P, G: Fn(*mut T) -> *mut E>
    GeneralMultiLinkedList<T, P, E, F, G>
{
    /// ### Safety
    ///
    /// It is unsafe to create multiple `MultiLinkedList`s over the same next field
    /// (the same `get_next_element_field` function)
    pub(crate) unsafe fn new(get_next_element_field: F, get_element_field: G) -> Self {
        Self {
            head: P::null(),
            get_next_element_field,
            get_element_field,
            _phantom_data: PhantomData,
        }
    }

    /// Return `true` if the list is empty
    pub(crate) fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Push `item` to the front of the list
    pub(crate) fn push(&mut self, item: &mut T) {
        let next = unsafe { (self.get_next_element_field)(item).as_mut().unwrap() };
        next.set(self.head.get());

        self.head.set(item);
    }

    /// Try to remove the first item in the list
    pub(crate) fn pop(&mut self) -> Option<*mut T> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head.get();
                let new_head =
                    unsafe { (self.get_next_element_field)(item).as_ref().unwrap() }.get();

                self.head.set(new_head);
                Some(item)
            }
        }
    }

    /// Returns an immutable iterator over the items in the list
    pub(crate) fn iter(&self) -> Iter<'_, '_, '_, T, P, E, F, G> {
        let head = if !self.head.is_null() {
            Some(unsafe { self.head.get().as_ref().unwrap() })
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
    pub(crate) fn iter_mut(&mut self) -> IterMut<'_, '_, T, P, E, F, G> {
        IterMut {
            curr: CurrItem {
                curr: self.head.get(),

                // don't advance prev for first item
                advance_prev: false,
                prev: &self.head,
                get_next: &self.get_next_element_field,
                get_element: &self.get_element_field,
                _phantom_data: PhantomData,
            },
        }
    }
}

/// An iterator over the linked list
pub(crate) struct Iter<
    'a,
    'b,
    'c,
    T,
    P: MultiLinkedListPointer<T>,
    E,
    F: Fn(*mut T) -> *mut P,
    G: Fn(*mut T) -> *mut E,
> {
    curr: Option<&'b T>,
    list: PhantomData<&'a GeneralMultiLinkedList<T, P, E, F, G>>,
    get_next_element: &'c F,
    get_element: &'c G,
}

impl<
        'a,
        'b,
        'c,
        T,
        P: MultiLinkedListPointer<T>,
        E,
        F: Fn(*mut T) -> *mut P,
        G: Fn(*mut T) -> *mut E,
    > Iterator for Iter<'a, 'b, 'c, T, P, E, F, G>
{
    type Item = &'b T;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr.take();

        if let Some(data) = ret {
            // some ugly pointer magic (because func only takes *mut pointers)
            // this is safe, because we cast the resulting *mut pointer back to a *const
            let ptr = (self.get_next_element)((data as *const T) as *mut T) as *const P;
            let next_ptr = unsafe { ptr.as_ref().unwrap().get() } as *const T;
            if !next_ptr.is_null() {
                self.curr = Some(unsafe { next_ptr.as_ref().unwrap() })
            }
        }

        ret
    }
}

pub(crate) struct DeleteHandle<
    'a,
    'b,
    'c,
    T,
    P: MultiLinkedListPointer<T>,
    E,
    F: Fn(*mut T) -> *mut P,
    G: Fn(*mut T) -> *mut E,
> {
    inner: &'c mut CurrItem<'a, 'b, T, P, E, F, G>,
}

impl<T, P: MultiLinkedListPointer<T>, E, F: Fn(*mut T) -> *mut P, G: Fn(*mut T) -> *mut E>
    DeleteHandle<'_, '_, '_, T, P, E, F, G>
{
    /// Delete an item
    #[inline]
    pub fn delete(self) {
        let new_ptr = (self.inner.get_next)(self.inner.curr);
        unsafe {
            let prev = self.inner.prev.as_ref().unwrap();
            let new = new_ptr.as_ref().unwrap();
            prev.set(new.get());
        };
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

struct CurrItem<
    'a,
    'b,
    T,
    P: MultiLinkedListPointer<T>,
    E,
    F: Fn(*mut T) -> *mut P,
    G: Fn(*mut T) -> *mut E,
> {
    prev: *const P,
    curr: *mut T,
    advance_prev: bool,
    get_next: &'b F,
    get_element: &'b G,
    _phantom_data: PhantomData<&'a T>,
}

pub(crate) struct IterMut<
    'a,
    'b,
    T,
    P: MultiLinkedListPointer<T>,
    E,
    F: Fn(*mut T) -> *mut P,
    G: Fn(*mut T) -> *mut E,
> {
    curr: CurrItem<'a, 'b, T, P, E, F, G>,
}

impl<
        'a,
        'b,
        T,
        P: MultiLinkedListPointer<T>,
        E,
        F: 'a + Fn(*mut T) -> *mut P,
        G: 'a + Fn(*mut T) -> *mut E,
    > IterMut<'a, 'b, T, P, E, F, G>
{
    pub fn next<'c>(&'c mut self) -> Option<DeleteHandle<'a, 'b, 'c, T, P, E, F, G>> {
        if self.curr.curr.is_null() {
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
            self.curr.curr = unsafe { self.curr.prev.as_ref().unwrap() }.get();

            if self.curr.curr.is_null() {
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
    const ENABLE_PRINTS: bool = false;

    use std::{collections::VecDeque, fmt::Debug, ptr::null_mut};

    use memoffset::offset_of;

    use super::{GeneralMultiLinkedList, MultiLinkedListDefaultPointer, MultiLinkedListPointer};

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

        fn get_next_x(ptr: *mut TestStruct) -> *mut MultiLinkedListDefaultPointer<TestStruct> {
            const OFFSET: usize = offset_of!(TestStruct, next_x);
            (unsafe { (ptr as *mut u8).add(OFFSET) })
                as *mut MultiLinkedListDefaultPointer<TestStruct>
        }

        fn get_next_y(ptr: *mut TestStruct) -> *mut MultiLinkedListDefaultPointer<TestStruct> {
            const OFFSET: usize = offset_of!(TestStruct, next_y);
            (unsafe { (ptr as *mut u8).add(OFFSET) })
                as *mut MultiLinkedListDefaultPointer<TestStruct>
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
        P: MultiLinkedListPointer<T>,
        E: Clone + Copy + PartialEq + Debug,
        F: Fn(*mut T) -> *mut P,
        G: Fn(*mut T) -> *mut E,
    > {
        check_list: VecDeque<T>,
        list: GeneralMultiLinkedList<T, P, E, F, G>,
    }

    impl<
            T: Clone + Copy + PartialEq + Debug,
            P: MultiLinkedListPointer<T>,
            E: Clone + Copy + PartialEq + Debug,
            F: Fn(*mut T) -> *mut P,
            G: Fn(*mut T) -> *mut E,
        > TestableModularLinkedList<T, P, E, F, G>
    {
        pub unsafe fn new(function: F, function2: G) -> Self {
            Self {
                check_list: VecDeque::new(),
                list: GeneralMultiLinkedList::new(function, function2),
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
            assert_eq!(self.list.iter().count(), self.check_list.len());

            for (a, b) in self.list.iter().zip(self.check_list.iter()) {
                if ENABLE_PRINTS {
                    println!("{:?} =?= {:?}", a, b);
                }

                if a != b {
                    assert!(a == b);
                }
            }
        }

        pub fn print(&self) {
            if ENABLE_PRINTS {
                print!("list: [");
                for x in self.list.iter() {
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
            if ENABLE_PRINTS {
                println!("### before iter_mut ###");
                list_x.print();
                println!("### iter_mut ###");
            }

            let mut iter = list_x.list.iter_mut();
            while let Some(mut handle) = iter.next() {
                let item = unsafe { handle.get_element() };
                let stop = item.a == 0;

                if item.b {
                    if ENABLE_PRINTS {
                        println!("delete {:?}", item);
                    }

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
            if ENABLE_PRINTS {
                println!("### before iter_mut ###");
                list_x.print();
                println!("### iter_mut ###");
            }

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
            if ENABLE_PRINTS {
                println!("### before iter_mut ###");
                list_y.print();
                println!("### iter_mut ###");
            }

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
            if ENABLE_PRINTS {
                println!("### before iter_mut ###");
                list_y.print();
                println!("### iter_mut ###");
            }

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
