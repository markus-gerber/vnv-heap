use core::{mem::MaybeUninit, sync::atomic::Ordering};
use memoffset::offset_of;

use super::{resident_object::ResidentObjectMetadata, ResidentObjectMetadataNonAtomic};
use crate::modules::persistent_storage::{persistent_storage_util, PersistentStorageModule};

const RESIDENT_PTR_UNUSED: usize = 0;

/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
pub(crate) struct ResidentObjectMetadataBackup {
    /// at which address does this data live
    /// (pointers could exist here so we need to restore
    /// the object at exactly the previous address)
    resident_ptr: usize,

    inner: MaybeUninit<ResidentObjectMetadataNonAtomic>,
}

impl ResidentObjectMetadataBackup {
    pub(crate) fn new_unused() -> Self {
        ResidentObjectMetadataBackup {
            resident_ptr: RESIDENT_PTR_UNUSED,
            inner: MaybeUninit::uninit(),
        }
    }

    pub(crate) fn from_metadata(metadata: &ResidentObjectMetadata) -> Self {
        let resident_ptr = unsafe {
            ResidentObjectMetadata::ptr_to_resident_obj_ptr_base(
                (metadata as *const ResidentObjectMetadata) as *mut ResidentObjectMetadata,
            )
        } as usize;

        ResidentObjectMetadataBackup {
            resident_ptr,
            inner: MaybeUninit::new(ResidentObjectMetadataNonAtomic {
                inner: metadata.inner.clone(),
                next_resident_object: metadata.next_resident_object.load(Ordering::SeqCst)
                    as *mut ResidentObjectMetadataNonAtomic,
            }),
        }
    }

    #[inline]
    pub(crate) fn is_unused(&self) -> bool {
        self.resident_ptr == RESIDENT_PTR_UNUSED
    }

    /// `offset` can be the offset where this metadata is stored
    pub(crate) fn to_metadata(&self) -> Option<&ResidentObjectMetadataNonAtomic> {
        if self.is_unused() {
            return None;
        }

        Some(unsafe { self.inner.assume_init_ref() })
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
}

impl Drop for ResidentObjectMetadataBackup {
    fn drop(&mut self) {
        if !self.is_unused() {
            unsafe { self.inner.assume_init_drop() };
        }
    }
}
