use core::{alloc::Layout, sync::atomic::AtomicPtr};
use memoffset::offset_of;

use super::{
    resident_object_status::ResidentObjectStatus,
    resident_object_metadata::{ResidentObjectMetadata, ResidentObjectMetadataInner},
};
use crate::modules::persistent_storage::{persistent_storage_util, PersistentStorageModule};

/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
pub(crate) struct ResidentObjectMetadataBackup {
    /// Next item in the resident object list
    /// This is either 0 or the offset of the next metadata in non volatile storage
    pub(crate) next_resident_object: usize,

    pub(crate) inner: ResidentObjectMetadataBackupInner,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentObjectMetadataBackupInner {
    /// What parts of this resident object are currently dirty?
    pub(crate) dirty_status: ResidentObjectStatus,

    /// Points to the location in RAM where this metadata object is stored
    pub(crate) offset: usize,

    pub(crate) layout: Layout,
}

impl ResidentObjectMetadataBackupInner {
    const fn default() -> Self {
        Self {
            dirty_status: ResidentObjectStatus::new_metadata_dirty(),
            offset: 0,
            layout: Layout::new::<()>(),
        }
    }

    fn from(value: &ResidentObjectMetadataInner) -> Self {
        let ResidentObjectMetadataInner {
            dirty_status,
            layout,
            offset: _offset,

            #[cfg(debug_assertions)]
                data_offset: _data_offset,
        } = value;

        Self {
            dirty_status: dirty_status.clone(),
            layout: layout.clone(),
            offset: (value as *const ResidentObjectMetadataInner) as usize,
        }
    }

    fn to_metadata(self, storage_offset: usize) -> ResidentObjectMetadataInner {
        let ResidentObjectMetadataBackupInner {
            dirty_status,
            layout,
            offset: _offset,
        } = self;

        ResidentObjectMetadataInner {
            dirty_status: dirty_status,
            layout: layout,
            offset: storage_offset,

            #[cfg(debug_assertions)]
            data_offset: usize::MAX,
        }
    }
}

impl ResidentObjectMetadataBackup {
    pub(crate) const fn new_unused() -> Self {
        ResidentObjectMetadataBackup {
            inner: ResidentObjectMetadataBackupInner::default(),
            next_resident_object: 0,
        }
    }

    pub(crate) fn from_metadata(
        metadata: &ResidentObjectMetadata,
        next_item_offset: usize,
    ) -> Self {
        let obj = ResidentObjectMetadataBackup {
            inner: ResidentObjectMetadataBackupInner::from(&metadata.inner),
            next_resident_object: next_item_offset,
        };

        obj
    }

    pub(crate) fn to_metadata(
        self,
        storage_offset: usize,
        next_resident_object: *mut ResidentObjectMetadata,
    ) -> ResidentObjectMetadata {
        ResidentObjectMetadata {
            inner: self.inner.to_metadata(storage_offset),
            next_resident_object: AtomicPtr::new(next_resident_object),
        }
    }

    pub(crate) unsafe fn flush_dirty_status<S: PersistentStorageModule>(
        data_offset: usize,
        dirty_status: &ResidentObjectStatus,
        storage: &mut S,
    ) -> Result<(), ()> {
        const DIRTY_STATUS_OFFSET: usize = offset_of!(ResidentObjectMetadataBackup, inner)
            + offset_of!(ResidentObjectMetadataBackupInner, dirty_status);

        persistent_storage_util::write_storage_data(
            storage,
            data_offset + DIRTY_STATUS_OFFSET,
            dirty_status,
        )
    }

    pub(crate) unsafe fn write_next_ptr<S: PersistentStorageModule>(
        data_offset: usize,
        next_offset: usize,
        storage: &mut S,
    ) -> Result<(), ()> {
        const NEXT_OBJ_OFFSET: usize =
            offset_of!(ResidentObjectMetadataBackup, next_resident_object);

        persistent_storage_util::write_storage_data(
            storage,
            data_offset + NEXT_OBJ_OFFSET,
            &next_offset,
        )
    }
}
