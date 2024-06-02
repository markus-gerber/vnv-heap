use crate::modules::persistent_storage::persistent_storage_util::{
    read_storage_data, write_storage_data,
};
use crate::modules::persistent_storage::PersistentStorageModule;
use core::{
    alloc::Layout,
    fmt::Debug,
    marker::PhantomData,
    mem::size_of,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct NonResidentLinkedListItemLocation {
    offset: usize,
}

impl NonResidentLinkedListItemLocation {
    fn from_base_offset(offset: usize) -> Self {
        Self { offset }
    }

    pub fn get_base_offset(&self) -> usize {
        self.offset
    }

    pub fn get_data_offset(&self) -> usize {
        self.offset + size_of::<usize>()
    }
}

pub struct NonResidentLinkedList<T: Sized> {
    head: usize,
    _phantom_data: PhantomData<T>,
}

/// Magic value that indicates that there is no next value in the free list
const NEXT_NULL: usize = usize::MAX;

/// Internal data representation, to save next pointer and data
///
/// **Important**: Don't remove `#[repr(C)]`
#[repr(C)]
struct NonResidentLinkedListItem<T> {
    next: usize,
    data: T,
}

impl<T: Sized> NonResidentLinkedList<T> {
    pub fn new() -> Self {
        Self {
            head: NEXT_NULL,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: Sized> NonResidentLinkedList<T> {
    /// The total size of an item stored in this list in persistent storage
    pub const fn total_item_size() -> usize {
        size_of::<NonResidentLinkedListItem<T>>()
    }

    pub const fn item_layout() -> Layout {
        Layout::new::<NonResidentLinkedListItem<T>>()
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head == NEXT_NULL
    }

    /// Push `item_offset` to the front of the list.
    ///
    /// `item_offset` is offset in bytes
    pub unsafe fn push<S: PersistentStorageModule>(
        &mut self,
        item_offset: usize,
        data: T,
        storage: &mut S,
    ) -> Result<(), ()> {
        // check that this new item does not overwrite an existing one
        debug_assert!(
            self.iter(storage).all(|item| {
                let (curr_offset, _) = item.unwrap();
                let size = NonResidentLinkedList::<T>::total_item_size();

                // 9 + 24 <= 32 && 32 + 24 <= 9
                (item_offset + size <= curr_offset.get_base_offset())
                    || (curr_offset.get_base_offset() + size <= item_offset)
            }),
            "Invalid offset! Item is going to be overwritten!"
        );

        debug_assert_ne!(item_offset, NEXT_NULL, "cannot push reserved offset value");

        let item = NonResidentLinkedListItem {
            next: self.head,
            data: data,
        };
        write_storage_data(storage, item_offset, &item)?;
        self.head = item_offset;

        Ok(())
    }

    /// Removes the first item in the list
    pub fn pop<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<Option<(NonResidentLinkedListItemLocation, T)>, ()> {
        match self.is_empty() {
            true => Ok(None),
            false => {
                // Advance head pointer
                let current_offset = self.head;
                let item = unsafe {
                    read_storage_data::<NonResidentLinkedListItem<T>, S>(storage, current_offset)?
                };
                self.head = item.next;

                // tell potential cache layers that this item is not needed anymore for now
                storage.forget_region(
                    current_offset,
                    NonResidentLinkedList::<T>::total_item_size(),
                );

                Ok(Some((
                    NonResidentLinkedListItemLocation::from_base_offset(current_offset),
                    item.data,
                )))
            }
        }
    }

    /// Removes all items where `function` returns `true`
    ///
    /// If `single_item = true` is set, the search for more
    /// items to remove is cancelled after one item was found
    ///
    /// Returns the amount of items that were removed
    pub fn remove_where<
        S: PersistentStorageModule,
        F: Fn((NonResidentLinkedListItemLocation, &T)) -> bool,
    >(
        &mut self,
        storage: &mut S,
        single_item: bool,
        function: F,
    ) -> Result<usize, ()> {
        let mut prev = NEXT_NULL;
        let mut curr = self.head;
        let mut counter = 0;

        while curr != NEXT_NULL {
            let curr_element =
                unsafe { read_storage_data::<NonResidentLinkedListItem<T>, S>(storage, curr)? };

            if function((
                NonResidentLinkedListItemLocation::from_base_offset(curr),
                &curr_element.data,
            )) {
                counter += 1;

                // remove current item
                if prev == NEXT_NULL {
                    // this is the first item in the list
                    // so to remove it, we need to update the head
                    self.head = curr_element.next;
                } else {
                    // this is not the first item in the list
                    // so we need to update the previous item, to remove it
                    write_storage_data(storage, prev, &curr_element.next)?;
                }

                // tell potential cache layers that this item is not needed anymore for now
                storage.forget_region(curr, NonResidentLinkedList::<T>::total_item_size());

                if single_item {
                    // found one item, stop looking for more
                    return Ok(counter);
                }

                // skip the current element
                curr = prev;
            }

            // advance
            prev = curr;
            curr = curr_element.next;
        }

        Ok(counter)
    }

    /// Return an iterator over the items in the list
    pub fn iter<'a, S: PersistentStorageModule>(&self, storage: &'a mut S) -> Iter<'a, S, T> {
        Iter {
            curr: self.head,
            list: PhantomData,
            storage: storage,
        }
    }

    /// Converts base offset of the `NonResidentLinkedListItem<T>` provided by
    /// `iter(...)` or `remove_where(...)` to the offset of `NonResidentLinkedListItem<T>.data`
    pub fn get_data_offset(base_offset: usize) -> usize {
        base_offset + size_of::<usize>()
    }
}

impl<T> Debug for NonResidentLinkedList<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.head))
    }
}

/// An iterator over the linked list
pub struct Iter<'a, S: PersistentStorageModule, T: Sized> {
    curr: usize,
    list: PhantomData<NonResidentLinkedList<T>>,
    storage: &'a mut S,
}

impl<'a, S: PersistentStorageModule, T: Sized> Iterator for Iter<'a, S, T> {
    type Item = Result<(NonResidentLinkedListItemLocation, T), ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr == NEXT_NULL {
            None
        } else {
            let item = self.curr;

            match unsafe {
                read_storage_data::<NonResidentLinkedListItem<T>, S>(self.storage, item)
            } {
                Err(()) => {
                    self.curr = NEXT_NULL;
                    Some(Err(()))
                }
                Ok(dest) => {
                    self.curr = dest.next;

                    Some(Ok((
                        NonResidentLinkedListItemLocation::from_base_offset(item),
                        dest.data,
                    )))
                }
            }
        }
    }
}

pub struct SimpleNonResidentLinkedList {
    inner: NonResidentLinkedList<()>,
}

impl SimpleNonResidentLinkedList {
    pub fn new() -> Self {
        Self {
            inner: NonResidentLinkedList::<()>::new(),
        }
    }

    /// The total size of an item stored in this list in persistent storage
    pub const fn total_item_size() -> usize {
        NonResidentLinkedList::<()>::total_item_size()
    }

    pub const fn item_layout() -> Layout {
        NonResidentLinkedList::<()>::item_layout()
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push `item_offset` to the front of the list.
    ///
    /// `item_offset` is offset in bytes
    pub unsafe fn push<S: PersistentStorageModule>(
        &mut self,
        item_offset: usize,
        storage: &mut S,
    ) -> Result<(), ()> {
        self.inner.push(item_offset, (), storage)
    }

    /// Removes the first item in the list
    pub fn pop<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<Option<usize>, ()> {
        Ok(self
            .inner
            .pop(storage)?
            .map(|(offset, _)| offset.get_base_offset()))
    }

    /// Return an iterator over the items in the list
    pub fn iter<'a, S: PersistentStorageModule>(&self, storage: &'a mut S) -> SimpleIter<'a, S> {
        SimpleIter {
            inner: self.inner.iter(storage),
        }
    }

    /// Removes all items where `function` returns `true`
    ///
    /// If `single_item = true` is set, the search for more
    /// items to remove is cancelled after one item was found
    ///
    /// Returns the amount of items that were removed
    pub fn remove_where<S: PersistentStorageModule, F: Fn(usize) -> bool>(
        &mut self,
        storage: &mut S,
        single_item: bool,
        function: F,
    ) -> Result<usize, ()> {
        self.inner
            .remove_where(storage, single_item, |(offset, _)| {
                function(offset.get_base_offset())
            })
    }
}

impl Debug for SimpleNonResidentLinkedList {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", self.inner))
    }
}

pub struct SimpleIter<'a, S: PersistentStorageModule> {
    inner: Iter<'a, S, ()>,
}

impl<'a, S: PersistentStorageModule> Iterator for SimpleIter<'a, S> {
    type Item = Result<usize, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|data| data.map(|(offset, _)| offset.get_base_offset()))
    }
}

pub struct AtomicPushOnlyNonResidentLinkedList<T: Sized> {
    head: AtomicUsize,
    _phantom_data: PhantomData<T>,
}

impl<T: Sized> AtomicPushOnlyNonResidentLinkedList<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicUsize::new(NEXT_NULL),
            _phantom_data: PhantomData,
        }
    }
}

impl<T: Sized> AtomicPushOnlyNonResidentLinkedList<T> {
    /// The total size of an item stored in this list in persistent storage
    pub const fn total_item_size() -> usize {
        size_of::<NonResidentLinkedListItem<T>>()
    }

    pub const fn item_layout() -> Layout {
        Layout::new::<NonResidentLinkedListItem<T>>()
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::SeqCst) == NEXT_NULL
    }

    /// Push `item_offset` to the front of the list.
    ///
    /// `item_offset` is offset in bytes
    pub unsafe fn push<S: PersistentStorageModule>(
        &self,
        item_offset: usize,
        data: T,
        storage: &mut S,
    ) -> Result<(), ()> {
        // check that this new item does not overwrite an existing one
        debug_assert!(
            self.iter(storage).all(|item| {
                let (curr_offset, _) = item.unwrap();
                let size = NonResidentLinkedList::<T>::total_item_size();

                // 9 + 24 <= 32 && 32 + 24 <= 9
                (item_offset + size <= curr_offset.get_base_offset())
                    || (curr_offset.get_base_offset() + size <= item_offset)
            }),
            "Invalid offset! Item is going to be overwritten!"
        );

        debug_assert_ne!(item_offset, NEXT_NULL, "cannot push reserved offset value");

        let item = NonResidentLinkedListItem {
            next: self.head.load(Ordering::SeqCst),
            data: data,
        };
        write_storage_data(storage, item_offset, &item)?;
        self.head.store(item_offset, Ordering::SeqCst);

        Ok(())
    }

    /// Return an iterator over the items in the list
    pub fn iter<'a, S: PersistentStorageModule>(&self, storage: &'a mut S) -> Iter<'a, S, T> {
        Iter {
            curr: self.head.load(Ordering::SeqCst),
            list: PhantomData,
            storage: storage,
        }
    }

    pub fn get_shared_head_ptr(&self) -> SharedAtomicLinkedListHeadPtr<T> {
        SharedAtomicLinkedListHeadPtr {
            phantom_data: PhantomData,
            ptr: &self.head,
        }
    }
}

pub struct SharedAtomicLinkedListHeadPtr<'a, T> {
    ptr: &'a AtomicUsize,
    phantom_data: PhantomData<T>,
}

impl<'a, T> SharedAtomicLinkedListHeadPtr<'a, T> {
    pub fn get_atomic_iter<'b, S: PersistentStorageModule>(
        &self,
        storage: &'b mut S,
    ) -> Iter<'b, S, T> {
        Iter {
            curr: self.ptr.load(Ordering::SeqCst),
            list: PhantomData,
            storage: storage,
        }
    }
}

impl<T> Debug for AtomicPushOnlyNonResidentLinkedList<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.head.load(Ordering::SeqCst)))
    }
}

#[cfg(test)]
mod test {
    use crate::modules::{
        nonresident_allocator::NonResidentLinkedList,
        persistent_storage::{
            persistent_storage_util::read_storage_data, test::get_test_storage,
            PersistentStorageModule,
        },
    };
    use std::{
        collections::VecDeque,
        io::{stdout, Write},
    };

    #[derive(PartialEq, Debug, Clone, Copy)]
    struct ListData {
        a: bool,
        b: u64,
    }

    #[cfg(not(no_std))]
    #[test]
    fn test_non_resident_linked_list_push_pop() {
        const TEST_SIZE: usize = 4096;
        const INIT_VAL: u8 = u8::MAX;
        const ITEM_SIZE: usize = NonResidentLinkedList::<ListData>::total_item_size();

        let mut storage = get_test_storage("test_non_resident_linked_list_push_pop", TEST_SIZE);

        let init_buffer = [INIT_VAL; TEST_SIZE];
        storage.write(0, &init_buffer).unwrap();

        let mut list: NonResidentLinkedList<ListData> = NonResidentLinkedList::new();
        let mut check_list: VecDeque<(usize, ListData)> = VecDeque::new();

        // lists should be empty
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        let data = [
            (32, ListData { a: false, b: 10 }),
            (4000, ListData { a: true, b: 10000 }),
            (128, ListData { a: true, b: 17 }),
            (3000, ListData { a: true, b: 32 }),
            (3000 + ITEM_SIZE, ListData { a: false, b: 7000 }),
            (512, ListData { a: true, b: 9001 }),
            (32 + ITEM_SIZE, ListData { a: false, b: 27 }),
        ];

        let holes: Vec<usize> = data.iter().map(|(offset, _)| offset.clone()).collect();

        for element in data {
            check_list.push_front(element.clone());
            unsafe { list.push(element.0, element.1, &mut storage).unwrap() };

            lists_assert_eq(&mut list, &mut storage, &mut check_list);
        }

        check_integrity(&mut storage, &holes, ITEM_SIZE, INIT_VAL, TEST_SIZE);

        while check_list.is_empty() && list.is_empty() {
            let (loc, data) = list.pop(&mut storage).unwrap().unwrap();
            assert_eq!(
                check_list.pop_front().unwrap(),
                (loc.get_base_offset(), data)
            );

            lists_assert_eq(&mut list, &mut storage, &mut check_list);
        }

        check_integrity(&mut storage, &holes, ITEM_SIZE, INIT_VAL, TEST_SIZE);
    }

    #[cfg(not(no_std))]
    #[test]
    fn test_non_resident_linked_list_remove_where() {
        const TEST_SIZE: usize = 4096;
        const INIT_VAL: u8 = u8::MAX;
        const ITEM_SIZE: usize = NonResidentLinkedList::<ListData>::total_item_size();

        let mut storage = get_test_storage("test_non_resident_linked_list_remove_where", TEST_SIZE);

        let init_buffer = [INIT_VAL; TEST_SIZE];
        storage.write(0, &init_buffer).unwrap();

        let mut list: NonResidentLinkedList<ListData> = NonResidentLinkedList::new();
        let mut check_list: VecDeque<(usize, ListData)> = VecDeque::new();

        // lists should be empty
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        let data = [
            (32, ListData { a: false, b: 10 }),
            (4000, ListData { a: true, b: 10000 }),
            (128, ListData { a: true, b: 17 }),
            (0, ListData { a: false, b: 16 }),
            (3000, ListData { a: true, b: 32 }),
            (3000 + ITEM_SIZE, ListData { a: false, b: 7000 }),
            (512, ListData { a: true, b: 9001 }),
            (32 + ITEM_SIZE, ListData { a: false, b: 27 }),
        ];

        let holes: Vec<usize> = data.iter().map(|(offset, _)| offset.clone()).collect();

        for element in data {
            check_list.push_front(element.clone());
            unsafe { list.push(element.0, element.1, &mut storage).unwrap() };

            lists_assert_eq(&mut list, &mut storage, &mut check_list);
        }

        check_integrity(&mut storage, &holes, ITEM_SIZE, INIT_VAL, TEST_SIZE);

        // should not change anything
        list.remove_where(&mut storage, false, |_| false).unwrap();
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        // should only remove one item
        list.remove_where(&mut storage, true, |(offset, _)| {
            offset.get_base_offset() >= 1000
        })
        .unwrap();
        check_list.retain(|(offset, _)| *offset != 3000 + ITEM_SIZE);
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        // remove multiple items based one ListData
        list.remove_where(&mut storage, false, |(_, data)| !data.a)
            .unwrap();
        check_list.retain(|(_, data)| data.a);
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        // remove all remaining items
        list.remove_where(&mut storage, false, |_| true).unwrap();
        check_list.retain(|_| false);
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        check_integrity(&mut storage, &holes, ITEM_SIZE, INIT_VAL, TEST_SIZE);
    }

    #[cfg(not(no_std))]
    #[test]
    fn test_non_resident_linked_list_filled() {
        const ITEM_SIZE: usize = NonResidentLinkedList::<ListData>::total_item_size();
        const ITEM_COUNT: usize = 100;
        const TEST_SIZE: usize = ITEM_SIZE * ITEM_COUNT;
        const INIT_VAL: u8 = u8::MAX;

        let mut storage = get_test_storage("test_non_resident_linked_list_filled", TEST_SIZE);

        let init_buffer = [INIT_VAL; TEST_SIZE];
        storage.write(0, &init_buffer).unwrap();

        let mut list: NonResidentLinkedList<ListData> = NonResidentLinkedList::new();
        let mut check_list: VecDeque<(usize, ListData)> = VecDeque::new();

        // lists should be empty
        lists_assert_eq(&mut list, &mut storage, &mut check_list);

        // skip 3 elements, when pushing so there is a
        // more random pattern than just pushing each item
        let skip_count: usize = 4;

        for i in 0..ITEM_COUNT {
            let offset = (i * ITEM_SIZE * skip_count) % TEST_SIZE
                + ((i * skip_count) / ITEM_COUNT) * ITEM_SIZE;
            let data = generate_list_data(i);

            check_list.push_front((offset, data.clone()));
            unsafe { list.push(offset, data, &mut storage).unwrap() };

            lists_assert_eq(&mut list, &mut storage, &mut check_list);
        }

        while check_list.is_empty() && list.is_empty() {
            let (loc, data) = list.pop(&mut storage).unwrap().unwrap();

            assert_eq!(
                check_list.pop_front().unwrap(),
                (loc.get_base_offset(), data)
            );

            lists_assert_eq(&mut list, &mut storage, &mut check_list);
        }
    }

    // semi random list data generator
    fn generate_list_data(i: usize) -> ListData {
        ListData {
            a: ((i % 3) == 0 || (i % 5 == 0)) && (i % 4) != 0,
            b: (((u64::MAX / 5).wrapping_mul(i as u64))
                .wrapping_add((u64::MAX / 2 - 1).wrapping_mul((i as u64) % 7)))
            .wrapping_add(2u64.pow(31) - 1),
        }
    }

    /// checks that no other values are changed (except from the holes)
    fn check_integrity<S: PersistentStorageModule>(
        storage: &mut S,
        holes: &Vec<usize>,
        hole_size: usize,
        initial_value: u8,
        test_size: usize,
    ) {
        let mut i = 0;
        while i < test_size {
            if holes.contains(&i) {
                // skip the next values
                i += hole_size;
                continue;
            }

            let data: u8 = unsafe { read_storage_data(storage, i).unwrap() };
            assert_eq!(data, initial_value);

            i += 1;
        }
    }

    fn lists_assert_eq<S: PersistentStorageModule>(
        nonresident_list: &mut NonResidentLinkedList<ListData>,
        storage: &mut S,
        check_list: &mut VecDeque<(usize, ListData)>,
    ) {
        assert_eq!(
            nonresident_list.iter(storage).count(),
            check_list.iter().count()
        );
        stdout().flush().unwrap();

        for (item1, item2) in nonresident_list.iter(storage).zip(check_list.iter()) {
            let item1 = item1.unwrap();
            assert_eq!(item1.0.get_base_offset(), item2.0);
            assert_eq!(item1.1, item2.1);
        }
    }
}
