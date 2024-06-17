use super::ResidentObjectMetadataBackup;
use crate::modules::{
    nonresident_allocator::{
        AtomicPushOnlyNonResidentLinkedList, Iter,
        SharedAtomicLinkedListHeadPtr,
    },
    persistent_storage::PersistentStorageModule,
};
use core::alloc::Layout;

pub(crate) struct MetadataBackupList {
    inner: AtomicPushOnlyNonResidentLinkedList<ResidentObjectMetadataBackup>,
    length: usize,
}

impl MetadataBackupList {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            inner: AtomicPushOnlyNonResidentLinkedList::new(),
            length: 0,
        }
    }

    /// The total size of an item stored in this list in persistent storage
    #[inline]
    #[allow(dead_code)]
    pub(crate) const fn total_item_size() -> usize {
        AtomicPushOnlyNonResidentLinkedList::<ResidentObjectMetadataBackup>::total_item_size()
    }

    #[inline]
    pub(crate) const fn item_layout() -> Layout {
        AtomicPushOnlyNonResidentLinkedList::<ResidentObjectMetadataBackup>::item_layout()
    }

    /// Return `true` if the list is empty
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push `item_offset` to the front of the list.
    ///
    /// `item_offset` is offset in bytes
    #[inline]
    pub(crate) unsafe fn push<S: PersistentStorageModule>(
        &mut self,
        item_offset: usize,
        data: ResidentObjectMetadataBackup,
        storage: &mut S,
    ) -> Result<(), ()> {
        let res = self.inner.push(item_offset, data, storage);
        if res.is_ok() {
            self.length += 1;
        }

        res
    }

    /// Return an iterator over the items in the list
    #[inline]
    pub(crate) fn iter<'a>(
        &'a self,
    ) -> Iter<'a, ResidentObjectMetadataBackup> {
        self.inner.iter()
    }

    #[inline]
    pub(crate) fn get_shared_head_ptr(&self) -> SharedMetadataBackupPtr<'_> {
        SharedMetadataBackupPtr {
            inner: self.inner.get_shared_head_ptr(),
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.length
    }
}

pub(crate) struct SharedMetadataBackupPtr<'a> {
    inner: SharedAtomicLinkedListHeadPtr<'a, ResidentObjectMetadataBackup>,
}
impl SharedMetadataBackupPtr<'_> {
    pub(crate) fn get_atomic_iter<'a>(
        &'a self,
    ) -> Iter<'a, ResidentObjectMetadataBackup> {
        self.inner.get_atomic_iter()
    }
}
