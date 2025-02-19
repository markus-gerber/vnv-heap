use core::{
    marker::PhantomData,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};

use super::resident_object_metadata::ResidentObjectMetadata;

pub(crate) struct ResidentList {
    head: AtomicPtr<ResidentObjectMetadata>,
}

impl ResidentList {
    pub(crate) fn new() -> Self {
        Self {
            head: AtomicPtr::new(null_mut()),
        }
    }

    /// Return `true` if the list is empty
    #[allow(unused)]
    pub(crate) fn is_empty(&self) -> bool {
        self.head.load(Ordering::SeqCst).is_null()
    }

    /// Inserts `item` to the list.
    /// 
    /// As all items are sorted based on their addresses, this will iterate over the list and insert
    /// it to the right place.
    ///
    /// ### Safety
    ///
    /// This is only safe if this list does not contain `item`.
    pub(crate) unsafe fn insert(&self, item: &mut ResidentObjectMetadata) {
        let new_addr = (item as *mut ResidentObjectMetadata) as usize;
        let mut prev = &self.head;
        let mut curr;

        loop {
            curr = prev.load(Ordering::SeqCst);
            if curr.is_null() {
                break;
            }

            if curr as usize > new_addr {
                break;
            }

            prev = &curr.as_ref().unwrap().next_resident_object;
        }

        item.next_resident_object.store(curr, Ordering::SeqCst);
        prev.store(item, Ordering::SeqCst);
    }

    /// Removes `item` from the list.
    /// 
    /// Returns Err if this item was not found in this list
    pub(crate) fn remove(&self, item: *mut ResidentObjectMetadata) -> Result<(), ()> {
        let mut iter = self.iter_mut();

        while let Some(mut curr) = iter.next() {
            if item == curr.get_element() {
                curr.delete();
                return Ok(());
            }
        }

        Err(())
    }

    /// Returns an immutable iterator over the items in the list
    pub(crate) fn iter(&self) -> Iter<'_, '_> {
        let ptr = self.head.load(Ordering::SeqCst);
        let head = unsafe { ptr.as_ref() };

        Iter {
            curr: head,
            list: PhantomData,
        }
    }

    /// Returns a mutable iterator over the items in the list
    pub(crate) fn iter_mut<'a>(&'a self) -> IterMut<'a> {
        IterMut {
            curr: CurrItem {
                curr: null_mut(),
                prev: &self.head,
            },
        }
    }

    pub(crate) fn get_shared_ref(&self) -> SharedResidentListRef {
        SharedResidentListRef {
            head: &self.head,
            _phantom_data: PhantomData,
        }
    }
}

pub(crate) struct SharedResidentListRef<'a> {
    head: *const AtomicPtr<ResidentObjectMetadata>,
    _phantom_data: PhantomData<&'a ()>,
}

impl SharedResidentListRef<'_> {
    /// Returns an immutable iterator over the items in the list
    pub(crate) fn iter(&self) -> Iter<'_, '_> {
        let ptr = unsafe { self.head.as_ref().unwrap().load(Ordering::SeqCst) };
        let head = unsafe { ptr.as_ref() };

        Iter {
            curr: head,
            list: PhantomData,
        }
    }

    pub(crate) fn get_head(&self) -> *const AtomicPtr<ResidentObjectMetadata> {
        self.head
    }
}

/// An iterator over the linked list
pub(crate) struct Iter<'a, 'b> {
    curr: Option<&'b ResidentObjectMetadata>,
    list: PhantomData<&'a ResidentList>,
}

impl<'a, 'b> Iterator for Iter<'a, 'b> {
    type Item = &'b ResidentObjectMetadata;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.curr.take();

        if let Some(data) = ret {
            // some ugly pointer magic (because func only takes *mut pointers)
            // this is safe, because we cast the resulting *mut pointer back to a *const
            let next_ptr = data.next_resident_object.load(Ordering::SeqCst);
            if !next_ptr.is_null() {
                self.curr = Some(unsafe { next_ptr.as_ref().unwrap() })
            }
        }

        ret
    }
}

pub(crate) struct DeleteHandle<'a, 'b> {
    inner: &'b mut CurrItem<'a>,
}

impl DeleteHandle<'_, '_> {
    /// Delete an item
    #[inline]
    pub(crate) fn delete<'a>(self) -> &'a mut ResidentObjectMetadata {
        let curr_ref = unsafe { self.inner.curr.as_mut().unwrap() };
        self.inner.curr = null_mut();

        let next_ptr = curr_ref.next_resident_object.load(Ordering::SeqCst);

        self.inner.prev.store(next_ptr, Ordering::SeqCst);

        curr_ref
    }

    #[inline]
    pub(crate) fn get_element<'a>(&'a mut self) -> &'a mut ResidentObjectMetadata {
        unsafe { self.inner.curr.as_mut().unwrap() }
    }
}

struct CurrItem<'a> {
    prev: &'a AtomicPtr<ResidentObjectMetadata>,
    curr: *mut ResidentObjectMetadata,
}

pub(crate) struct IterMut<'a> {
    curr: CurrItem<'a>,
}

impl<'a> IterMut<'a> {
    pub fn next<'b>(&'b mut self) -> Option<DeleteHandle<'a, 'b>> {
        if let Some(curr_ref) = unsafe { self.curr.curr.as_mut() } {
            // advance prev
            // we don't do this here if this is the first item
            // or if curr was deleted before (in both cases self.curr.curr is None)
            self.curr.prev = &curr_ref.next_resident_object;
        }

        // update current reference even if we don't advance prev
        // do this here because we have to be sure that there is no reference of curr.curr anymore
        self.curr.curr = self.curr.prev.load(Ordering::SeqCst);

        if self.curr.curr.is_null() {
            None
        } else {
            Some(DeleteHandle {
                inner: &mut self.curr,
            })
        }
    }
}

#[cfg(test)]
mod test {
    const ENABLE_PRINTS: bool = false;

    use crate::resident_object_manager::{
        resident_object_status::ResidentObjectStatus,
        resident_object_metadata::{ResidentObjectMetadata, ResidentObjectMetadataInner},
    };
    use std::{
        fmt::Debug,
        sync::atomic::{AtomicPtr, Ordering},
    };

    use super::ResidentList;

    impl Debug for ResidentObjectMetadata {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ResidentObjectMetadataNew")
                .field("inner", &self.inner)
                .field("next_resident_object", &self.next_resident_object)
                .finish()
        }
    }
    impl Debug for ResidentObjectMetadataInner {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ResidentObjectMetadataInner")
                .field("is_data", &self.status.is_data_dirty())
                .field("offset", &self.offset)
                .field("layout", &self.layout)
                .finish()
        }
    }

    impl PartialEq for ResidentObjectMetadata {
        fn eq(&self, other: &Self) -> bool {
            self.inner == other.inner
        }
    }

    impl PartialEq for ResidentObjectMetadataInner {
        fn eq(&self, other: &Self) -> bool {
            self.status == other.status
                && self.offset == other.offset
                && self.layout == other.layout
        }
    }

    impl Default for ResidentObjectMetadata {
        fn default() -> Self {
            Self {
                inner: Default::default(),
                next_resident_object: Default::default(),
            }
        }
    }

    impl Clone for ResidentObjectMetadata {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                next_resident_object: AtomicPtr::new(
                    self.next_resident_object.load(Ordering::SeqCst),
                ),
            }
        }
    }

    fn get_meta(offset: usize, is_data_dirty: bool) -> ResidentObjectMetadata {
        let mut dirty_status = ResidentObjectStatus::new_metadata_dirty(false);
        dirty_status.set_data_dirty(is_data_dirty);

        ResidentObjectMetadata {
            inner: ResidentObjectMetadataInner {
                status: dirty_status,
                offset,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    struct TestableModularLinkedList {
        check_list: Vec<ResidentObjectMetadata>,
        list: ResidentList,
    }

    impl TestableModularLinkedList {
        pub fn new() -> Self {
            Self {
                check_list: Vec::new(),
                list: ResidentList::new(),
            }
        }

        pub fn insert(&mut self, item: &mut ResidentObjectMetadata) {
            {
                let check_item = item.clone();
                check_item.next_resident_object.store(item, Ordering::SeqCst);
                self.check_list.push(check_item);
            }
            self.check_list.sort_by_key(|x| x.next_resident_object.load(Ordering::SeqCst) as usize);
            unsafe { self.list.insert(item) };

            self.check_integrity();
        }

        pub fn remove(&mut self, index: usize) -> Result<(), ()> {
            
            let (ptr, item) = {
                let mut iter = self.list.iter();
                for _ in 0..index {
                    iter.next().unwrap();
                }
                let item = iter.next().unwrap();
                let copy = item.clone();
                ((item as *const ResidentObjectMetadata) as *mut ResidentObjectMetadata, copy)
            };

            let item2 = self.check_list.remove(index);
            assert_eq!(item, item2);

            self.list.remove(ptr)?;

            self.check_integrity();
            Ok(())
        }

        pub fn check_integrity(&self) {
            if ENABLE_PRINTS {
                println!("\nCHECK_INTEGRITY");

            }

            assert_eq!(self.list.iter().count(), self.check_list.len());
            let mut counter = 0;

            for (a, b) in self.list.iter().zip(self.check_list.iter()) {
                if counter == 1000 {
                    panic!("endless loop detected!");
                }
                counter += 1;
                if ENABLE_PRINTS {
                    println!("{:?} =?= {:?}", a, b);
                }
                if a != b {
                    assert_eq!(a, b);
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
        let mut data = [
            get_meta(12, false),
            get_meta(1421, false),
            get_meta(0, true),
            get_meta(39, false),
            get_meta(1, true),
            get_meta(121983, true),
            get_meta(11, true),
            get_meta(24, false),
            get_meta(1, true),
        ];

        let mut list = TestableModularLinkedList::new();

        macro_rules! check_integrity {
            () => {{
                list.check_integrity();
            }};
        }

        list.insert(&mut data[1]);
        check_integrity!();
        list.insert(&mut data[5]);
        check_integrity!();
        list.insert(&mut data[4]);
        check_integrity!();
        list.insert(&mut data[3]);
        check_integrity!();
        list.insert(&mut data[2]);
        check_integrity!();
        list.insert(&mut data[8]);
        check_integrity!();

        // list at this point: [1,2,3,4,5,8]

        list.remove(0).unwrap();
        check_integrity!();
        
        // list at this point: [2,3,4,5,8]
        list.remove(4).unwrap();
        check_integrity!();
        
        // list at this point: [2,3,4,5]
        list.remove(0).unwrap();
        check_integrity!();
        
        // list at this point: [3,4,5]
        list.remove(1).unwrap();
        check_integrity!();
        
        // list at this point: [3,5]
        
        list.insert(&mut data[1]);
        check_integrity!();

        // list at this point: [1,3,5]
        list.insert(&mut data[8]);
        check_integrity!();

        // list at this point: [1,3,5,8]
        list.insert(&mut data[2]);
        check_integrity!();
        
        // list at this point: [1,2,3,5,8]

        {
            if ENABLE_PRINTS {
                println!("### before iter_mut ###");
                list.print();
                println!("### iter_mut ###");
            }
            let mut iter = list.list.iter_mut();
            while let Some(mut handle) = iter.next() {
                let item = handle.get_element();
                let stop = item.inner.offset == 0;

                if item.inner.status.is_data_dirty() {
                    if ENABLE_PRINTS {
                        println!("delete {:?}", item);
                    }
                    handle.delete();
                }

                if stop {
                    break;
                }
            }

            list.check_list = list
                .check_list
                .into_iter()
                .fold(
                    (Vec::new(), false),
                    |mut acc: (Vec<ResidentObjectMetadata>, bool), item| {
                        if acc.1 {
                            acc.0.push(item);
                            acc
                        } else {
                            if !item.inner.status.is_data_dirty() {
                                acc.0.push(item.clone())
                            }
                            if item.inner.offset == 0 {
                                acc.1 = true;
                            }
                            acc
                        }
                    },
                )
                .0;

            list.print();
        }
        check_integrity!();

        {
            if ENABLE_PRINTS {
                println!("### before iter_mut ###");
                list.print();
                println!("### iter_mut ###");
            }
            let mut iter = list.list.iter_mut();
            while let Some(handle) = iter.next() {
                handle.delete();
                break;
            }
            list.check_list.remove(0);
            list.print();
        }
        check_integrity!();
    }

    #[test]
    fn test_simple_filled() {
        let mut data = [
            get_meta(12, false),
            get_meta(1421, false),
            get_meta(1, true),
            get_meta(39, false),
            get_meta(0, true),
            get_meta(121983, true),
            get_meta(11, true),
            get_meta(24, false),
            get_meta(1, true),
        ];

        let mut list = TestableModularLinkedList::new();

        macro_rules! check_integrity {
            () => {{
                list.check_integrity();
            }};
        }

        list.insert(&mut data[7]);
        check_integrity!();
        list.insert(&mut data[6]);
        check_integrity!();
        list.insert(&mut data[3]);
        check_integrity!();
        list.insert(&mut data[2]);
        check_integrity!();
        list.insert(&mut data[5]);
        check_integrity!();
        list.insert(&mut data[4]);
        check_integrity!();
        list.insert(&mut data[8]);
        check_integrity!();
        list.insert(&mut data[1]);
        check_integrity!();
        list.insert(&mut data[0]);
        check_integrity!();

        for _ in 0..9 {
            list.remove(0).unwrap();
            check_integrity!();
        }
    }
}
