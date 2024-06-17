use core::{ptr::null_mut, sync::atomic::{Ordering, AtomicPtr}, alloc::Layout};
use memoffset::offset_of;

use super::{
    dirty_status::DirtyStatus, metadata_backup_info::MetadataBackupInfo,
    resident_object::ResidentObjectMetadata, ResidentObjectMetadataInner,
};
use crate::modules::persistent_storage::{persistent_storage_util, PersistentStorageModule};

const RESIDENT_PTR_UNUSED: usize = 0;

/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
pub(crate) struct ResidentObjectMetadataBackup {
    /// at which address does this data live
    /// (pointers could exist here so we need to restore
    /// the object at exactly the previous address)
    pub(crate) resident_ptr: usize,

    /// Next item in the resident object list
    pub(crate) next_resident_object: *mut ResidentObjectMetadata,

    pub(crate) inner: ResidentObjectMetadataBackupInner,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentObjectMetadataBackupInner {
    /// What parts of this resident object are currently dirty?
    pub(crate) dirty_status: DirtyStatus,

    /// Counts the amount of references that are currently held
    /// be the program
    pub(crate) ref_cnt: usize,

    pub(crate) offset: usize,

    pub(crate) layout: Layout,

    /// Offset of the metadata backup node that is being used.
    pub(crate) metadata_backup_node: MetadataBackupInfo,
}

impl ResidentObjectMetadataBackupInner {
    const fn default() -> Self {
        Self {
            dirty_status: DirtyStatus::new_metadata_dirty(),
            ref_cnt: 0,
            offset: 0,
            layout: Layout::new::<()>(),
            metadata_backup_node: MetadataBackupInfo::empty(),
        }
    }

    fn from(value: &ResidentObjectMetadataInner) -> Self {
        let ResidentObjectMetadataInner{
            dirty_status,
            layout,
            metadata_backup_node,
            offset,
            ref_cnt,
            
            #[cfg(debug_assertions)]
            data_offset: _data_offset,
        } = value;

        Self {
            dirty_status: dirty_status.clone(),
            layout: layout.clone(),
            metadata_backup_node: metadata_backup_node.clone(),
            offset: offset.clone(),
            ref_cnt: ref_cnt.clone()
        }
    }

    fn to_metadata(&self) -> ResidentObjectMetadataInner {
        let ResidentObjectMetadataBackupInner{
            dirty_status,
            layout,
            metadata_backup_node,
            offset,
            ref_cnt,
        } = self;

        ResidentObjectMetadataInner {
            dirty_status: dirty_status.clone(),
            layout: layout.clone(),
            metadata_backup_node: metadata_backup_node.clone(),
            offset: offset.clone(),
            ref_cnt: ref_cnt.clone(),

            #[cfg(debug_assertions)]
            data_offset: usize::MAX
        }
    }
}


impl ResidentObjectMetadataBackup {
    pub(crate) const fn new_unused() -> Self {
        ResidentObjectMetadataBackup {
            resident_ptr: RESIDENT_PTR_UNUSED,
            inner: ResidentObjectMetadataBackupInner::default(),
            next_resident_object: null_mut(),
        }
    }

    pub(crate) fn from_metadata(metadata: &ResidentObjectMetadata) -> Self {
        let resident_ptr = unsafe {
            ResidentObjectMetadata::ptr_to_resident_obj_ptr_base(
                (metadata as *const ResidentObjectMetadata) as *mut ResidentObjectMetadata,
            )
        } as usize;

        let obj = ResidentObjectMetadataBackup {
            resident_ptr,
            inner: ResidentObjectMetadataBackupInner::from(&metadata.inner),
            next_resident_object: metadata.next_resident_object.load(Ordering::SeqCst),
        };

        obj
    }

    pub(crate) fn to_metadata(&self) -> ResidentObjectMetadata {
        ResidentObjectMetadata {
            inner: self.inner.to_metadata(),
            next_resident_object: AtomicPtr::new(self.next_resident_object)
        }
    }

    #[inline]
    pub(crate) fn is_unused(&self) -> bool {
        self.resident_ptr == RESIDENT_PTR_UNUSED
    }

    /// Makes a `ResidentObjectMetadataBackup` unused that is stored persistently.
    ///
    /// ### Safety
    ///
    /// This call is only valid if `data_offset` points to a valid `ResidentObjectMetadataBackup` object
    pub(crate) unsafe fn make_unused<S: PersistentStorageModule>(
        data_offset: usize,
        storage: &mut S,
    ) -> Result<(), ()> {
        const RESIDENT_PTR_OFFSET: usize = offset_of!(ResidentObjectMetadataBackup, resident_ptr);

        persistent_storage_util::write_storage_data(
            storage,
            data_offset + RESIDENT_PTR_OFFSET,
            &RESIDENT_PTR_UNUSED,
        )
    }

    pub(crate) unsafe fn flush_dirty_status<S: PersistentStorageModule>(data_offset: usize, dirty_status: &DirtyStatus, storage: &mut S) -> Result<(), ()> {
        const DIRTY_STATUS_OFFSET: usize = offset_of!(ResidentObjectMetadataBackup, inner) + offset_of!(ResidentObjectMetadataBackupInner, dirty_status);

        persistent_storage_util::write_storage_data(
            storage,
            data_offset + DIRTY_STATUS_OFFSET,
            dirty_status,
        )
    }
}
