use core::{
    marker::PhantomData,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};

use super::resident_object::ResidentObjectMetadata;

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
    pub(crate) fn is_empty(&self) -> bool {
        self.head.load(Ordering::SeqCst).is_null()
    }

    /// Push `item` to the front of the list
    ///
    /// ### Safety
    ///
    /// This is only safe if `item` was not previously pushed to any list before (pushing after popping an item is okay tho).
    pub(crate) unsafe fn push(&mut self, item: &mut ResidentObjectMetadata) {
        let old_head = self.head.load(Ordering::SeqCst);
        item.next_resident_object.store(old_head, Ordering::SeqCst);
        self.head.store(item, Ordering::SeqCst);
    }

    /// Try to remove the first item in the list
    pub(crate) fn pop(&mut self) -> Option<*mut ResidentObjectMetadata> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head.load(Ordering::SeqCst);
                let new_head = unsafe { item.as_ref().unwrap() }
                    .next_resident_object
                    .load(Ordering::SeqCst);

                self.head.store(new_head, Ordering::SeqCst);
                Some(item)
            }
        }
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
    pub(crate) fn iter_mut<'a>(&'a mut self) -> IterMut<'a> {
        IterMut {
            curr: CurrItem {
                curr: null_mut(),
                prev: &self.head,
            },
        }
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
    use crate::resident_object_manager::{
        dirty_status::DirtyStatus,
        metadata_backup_info::MetadataBackupInfo,
        resident_object::{ResidentObjectMetadata, ResidentObjectMetadataInner},
    };
    use std::{
        alloc::Layout,
        collections::VecDeque,
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
                .field("is_data", &self.dirty_status.is_data_dirty())
                .field("ref_cnt", &self.ref_cnt)
                .field("offset", &self.offset)
                .field("layout", &self.layout)
                .field("data_offset", &self.data_offset)
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
            self.dirty_status == other.dirty_status
                && self.ref_cnt == other.ref_cnt
                && self.offset == other.offset
                && self.layout == other.layout
                && self.data_offset == other.data_offset
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
    impl Default for ResidentObjectMetadataInner {
        fn default() -> Self {
            Self {
                dirty_status: Default::default(),
                ref_cnt: Default::default(),
                offset: Default::default(),
                layout: Layout::new::<()>(),
                metadata_backup_node: MetadataBackupInfo::empty(),

                #[cfg(debug_assertions)]
                data_offset: usize::MAX,
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
        let mut dirty_status = DirtyStatus::new_metadata_dirty();
        dirty_status.set_data_dirty(is_data_dirty);

        ResidentObjectMetadata {
            inner: ResidentObjectMetadataInner {
                dirty_status,
                offset,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    struct TestableModularLinkedList {
        check_list: VecDeque<ResidentObjectMetadata>,
        list: ResidentList,
    }

    impl TestableModularLinkedList {
        pub fn new() -> Self {
            Self {
                check_list: VecDeque::new(),
                list: ResidentList::new(),
            }
        }

        pub fn push(&mut self, item: &mut ResidentObjectMetadata) {
            self.check_list.push_front(item.clone());
            unsafe { self.list.push(item) };

            self.check_integrity();
        }

        pub fn pop(&mut self) -> Option<*mut ResidentObjectMetadata> {
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
                println!("{:?} =?= {:?}", a, b);
                if a != b {
                    assert!(a == b);
                }
            }
        }

        pub fn print(&self) {
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

        list.push(&mut data[1]);
        check_integrity!();
        list.push(&mut data[5]);
        check_integrity!();
        list.push(&mut data[4]);
        check_integrity!();
        list.push(&mut data[3]);
        check_integrity!();
        list.push(&mut data[2]);
        check_integrity!();
        list.push(&mut data[8]);
        check_integrity!();

        list.pop();
        check_integrity!();
        list.pop();
        check_integrity!();
        list.pop();
        check_integrity!();

        list.push(&mut data[3]);
        check_integrity!();
        list.push(&mut data[2]);
        check_integrity!();
        list.push(&mut data[8]);
        check_integrity!();

        {
            println!("### before iter_mut ###");
            list.print();
            println!("### iter_mut ###");
            let mut iter = list.list.iter_mut();
            while let Some(mut handle) = iter.next() {
                let item = handle.get_element();
                let stop = item.inner.offset == 0;

                if item.inner.dirty_status.is_data_dirty() {
                    println!("delete {:?}", item);
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
                    (VecDeque::new(), false),
                    |mut acc: (VecDeque<ResidentObjectMetadata>, bool), item| {
                        if acc.1 {
                            acc.0.push_back(item);
                            acc
                        } else {
                            if !item.inner.dirty_status.is_data_dirty() {
                                acc.0.push_back(item.clone())
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
            println!("### before iter_mut ###");
            list.print();
            println!("### iter_mut ###");
            let mut iter = list.list.iter_mut();
            while let Some(handle) = iter.next() {
                handle.delete();
                break;
            }
            list.check_list.pop_front();
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

        list.push(&mut data[7]);
        check_integrity!();
        list.push(&mut data[6]);
        check_integrity!();
        list.push(&mut data[3]);
        check_integrity!();
        list.push(&mut data[2]);
        check_integrity!();
        list.push(&mut data[5]);
        check_integrity!();
        list.push(&mut data[4]);
        check_integrity!();
        list.push(&mut data[8]);
        check_integrity!();
        list.push(&mut data[1]);
        check_integrity!();
        list.push(&mut data[0]);
        check_integrity!();

        for _ in 0..9 {
            list.pop();
            check_integrity!();
        }
    }
}
