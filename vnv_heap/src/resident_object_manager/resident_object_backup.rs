use core::alloc::Layout;

use super::resident_object::{MetadataBackupInfo, ResidentObjectMetadataInner};


/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
pub(super) struct ResidentObjectMetadataBackup {
    /// size of the object
    layout: Layout,

    /// where is this objects stored inside of
    /// persistent storage
    offset: usize,

    /// how many references are there
    ref_cnt: usize,

    /// at which address does this data live
    /// (pointers could exist here so we need to restore
    /// the object at exactly the previous address)
    resident_ptr: usize,
}

impl ResidentObjectMetadataBackup {
    pub(crate) fn new_unused() -> Self {
        ResidentObjectMetadataBackup {
            layout: Layout::from_size_align(0, 1).unwrap(),
            offset: 0,
            ref_cnt: 0,
            resident_ptr: 0,
        }
    }

    pub(super) fn from_metadata(metadata: &ResidentObjectMetadataInner) -> Self {
        let resident_ptr = unsafe {
            ResidentObjectMetadataInner::ptr_to_resident_obj_ptr_base((metadata as *const ResidentObjectMetadataInner) as *mut ResidentObjectMetadataInner)
        } as usize;

        ResidentObjectMetadataBackup {
            offset: metadata.offset,
            ref_cnt: metadata.ref_cnt,
            resident_ptr: resident_ptr,
            layout: metadata.layout
        }
    }

    pub(super) fn is_unused(&self) -> bool {
        self.resident_ptr == 0
    }

    /// `offset` can be the offset where this metadata is stored
    pub(super) fn to_metadata(&self, backup_offset: Option<usize>) -> ResidentObjectMetadataInner {
        let backup_info = if let Some(backup_offset) = backup_offset {
            // TODO this has to depend on is_dirty!!!
            MetadataBackupInfo::new(backup_offset)
        } else {
            MetadataBackupInfo::empty()
        };
        
        ResidentObjectMetadataInner {
            #[cfg(debug_assertions)]
            data_offset: usize::MAX,
            is_dirty: false,
            // TODO figure out what to do here
            layout: self.layout,
            offset: self.offset,
            ref_cnt: self.ref_cnt,
            metadata_backup_node: backup_info
        }
    }
}