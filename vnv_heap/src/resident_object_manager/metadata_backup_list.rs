use core::alloc::Layout;
use crate::modules::{nonresident_allocator::{AtomicPushOnlyNonResidentLinkedList, Iter, NonResidentLinkedList, SharedAtomicLinkedListHeadPtr}, persistent_storage::PersistentStorageModule};
use super::ResidentObjectMetadataBackup;


pub(super) struct MetadataBackupList {
    inner: AtomicPushOnlyNonResidentLinkedList<ResidentObjectMetadataBackup>,
    length: usize,
}

impl MetadataBackupList {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            inner: AtomicPushOnlyNonResidentLinkedList::new(),
            length: 0,
        }
    }

    /// The total size of an item stored in this list in persistent storage
    #[inline]
    pub(super) const fn total_item_size() -> usize {
        AtomicPushOnlyNonResidentLinkedList::<ResidentObjectMetadataBackup>::total_item_size()
    }

    #[inline]
    pub(super) const fn item_layout() -> Layout {
        AtomicPushOnlyNonResidentLinkedList::<ResidentObjectMetadataBackup>::item_layout()
    }

    /// Return `true` if the list is empty
    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Push `item_offset` to the front of the list.
    ///
    /// `item_offset` is offset in bytes
    #[inline]
    pub(super) unsafe fn push<S: PersistentStorageModule>(
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
    pub(super) fn iter<'a, S: PersistentStorageModule>(&self, storage: &'a mut S) -> Iter<'a, S, ResidentObjectMetadataBackup> {
        self.inner.iter(storage)
    }

    #[inline]
    pub(super) fn get_shared_head_ptr(&self) -> SharedAtomicLinkedListHeadPtr<ResidentObjectMetadataBackup> {
        self.inner.get_shared_head_ptr()
    }

    #[inline]
    pub(super) fn len(&self) -> usize {
        self.length
    }

    /// Converts base offset of the `NonResidentLinkedListItem<T>` provided by
    /// `iter(...)` or `remove_where(...)` to the offset of `NonResidentLinkedListItem<T>.data`
    #[inline]
    pub fn get_data_offset(base_offset: usize) -> usize {
        NonResidentLinkedList::<ResidentObjectMetadataBackup>::get_data_offset(base_offset)
    }
}

pub(crate) struct SharedMetadataBackupPtr<'a> {
    inner: SharedAtomicLinkedListHeadPtr<'a, ResidentObjectMetadataBackup>
}
impl SharedMetadataBackupPtr<'_> {
    pub(super) fn get_atomic_iter<'a, S: PersistentStorageModule>(&self, storage: &'a mut S) -> Iter<'a, S, ResidentObjectMetadataBackup> {
        self.get_atomic_iter(storage)
    }
}
