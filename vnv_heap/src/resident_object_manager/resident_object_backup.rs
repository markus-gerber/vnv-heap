use core::{
    alloc::Layout,
    mem::size_of,
    ptr::{slice_from_raw_parts, slice_from_raw_parts_mut},
    sync::atomic::AtomicPtr,
};

use self::persistent_storage_util::read_storage_data;

use super::{
    partial_dirtiness_tracking::PartialDirtinessTrackingInfo,
    resident_object_metadata::{ResidentObjectMetadata, ResidentObjectMetadataInner},
    resident_object_status::ResidentObjectStatus,
};
use crate::{
    modules::persistent_storage::{persistent_storage_util, PersistentStorageModule},
    resident_object_manager::partial_dirtiness_tracking::MAX_SUPPORTED_PARTIAL_DIRTY_BUF_SIZE,
};

pub(crate) const TOTAL_METADATA_BACKUP_SIZE: usize = {
    size_of::<ResidentObjectMetadataBackup>()
};

pub(crate) const fn calc_backup_obj_layout_static<T>(
    use_partial_dirtiness_tracking: bool,
) -> (Layout, usize) {
    let dirtiness_buf_size = if use_partial_dirtiness_tracking {
        let (_, byte_count) = PartialDirtinessTrackingInfo::calc_bit_and_byte_count(size_of::<T>());

        byte_count
    } else {
        0
    };

    assert!(Layout::from_size_align(
        dirtiness_buf_size + TOTAL_METADATA_BACKUP_SIZE + size_of::<T>(),
        1
    )
    .is_ok());
    let layout = unsafe {
        Layout::from_size_align_unchecked(
            dirtiness_buf_size + TOTAL_METADATA_BACKUP_SIZE + size_of::<T>(),
            1,
        )
    };

    (layout, dirtiness_buf_size)
}

#[inline]
pub(crate) const fn calc_backup_obj_user_data_offset() -> usize {
    TOTAL_METADATA_BACKUP_SIZE
}

/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
///
/// IMPORTANT: DO NOT REORDER THE FIELDS OF THIS STRUCT AS IT IS CRUCIAL TO THE CURRENT IMPLEMENTATION
///
/// ALSO NOTE: ONLY REMOVE/ADD FIELDS WITH UPDATING:
/// TOTAL_METADATA_BACKUP_SIZE, GENERAL_METADATA_BACKUP_SIZE and METADATA_EXCEPT_GENERAL_BACKUP_SIZE
#[repr(C, packed(1))]
pub(crate) struct ResidentObjectMetadataBackup {
    /// Next item in the resident object list
    /// This is either 0 or the offset of the next metadata in non volatile storage
    pub(crate) next_resident_object: usize,

    pub(crate) inner: ResidentObjectMetadataBackupInner,
}

/// IMPORTANT: DO NOT REORDER THE FIELDS OF THIS STRUCT AS IT IS CRUCIAL TO THE CURRENT IMPLEMENTATION
#[derive(Clone, Copy)]
#[repr(C, packed(1))]
pub(crate) struct ResidentObjectMetadataBackupInner {
    /// What status is the resident object in?
    pub(crate) status: ResidentObjectStatus,

    /// Points to the location in RAM where this metadata object is stored
    pub(crate) offset: usize,

    pub(crate) layout: Layout,
}

impl ResidentObjectMetadataBackupInner {
    fn from(value: &ResidentObjectMetadataInner) -> Self {
        let ResidentObjectMetadataInner {
            status: dirty_status,
            layout,
            offset: _offset,
            partial_dirtiness_tracking_info: _partial_dirtiness_tracking_info,

            #[cfg(debug_assertions)]
                data_offset: _data_offset,
        } = value;

        Self {
            status: dirty_status.clone(),
            layout: layout.clone(),
            offset: (value as *const ResidentObjectMetadataInner) as usize,
        }
    }

    fn to_metadata(self, storage_offset: usize) -> ResidentObjectMetadataInner {
        let ResidentObjectMetadataBackupInner {
            status,
            layout,
            offset: _offset,
        } = self;

        let partial_dirtiness_tracking_info = if status.is_partial_dirtiness_tracking_enabled() {
            PartialDirtinessTrackingInfo::new_used_dynamic(&layout)
        } else {
            PartialDirtinessTrackingInfo::new_unused()
        };

        ResidentObjectMetadataInner {
            status,
            partial_dirtiness_tracking_info,
            layout: layout,
            offset: storage_offset,

            #[cfg(debug_assertions)]
            data_offset: usize::MAX,
        }
    }
}

impl ResidentObjectMetadataBackup {
    fn from_metadata(metadata: &ResidentObjectMetadata, next_item_offset: usize) -> Self {
        let obj = ResidentObjectMetadataBackup {
            inner: ResidentObjectMetadataBackupInner::from(&metadata.inner),
            next_resident_object: next_item_offset,
        };

        obj
    }

    fn to_metadata(
        self,
        storage_offset: usize,
        next_resident_object: *mut ResidentObjectMetadata,
    ) -> ResidentObjectMetadata {
        ResidentObjectMetadata {
            inner: self.inner.to_metadata(storage_offset),
            next_resident_object: AtomicPtr::new(next_resident_object),
        }
    }
}

/// Persists metadata including dirty bit list (if partial dirtiness tracking is enabled)
///
/// # Safety
///
/// If unsafe_include_data_inplace is set, the metadata and data will be written in **one** write call
/// This is done by overwriting the metadata with its packed representation
///
/// **Important**: This function will not restore the original state of the metadata!
#[inline]
pub(crate) unsafe fn persist_whole_metadata<S: PersistentStorageModule>(
    metadata: *mut ResidentObjectMetadata,
    next_object_offset: usize,
    storage: &mut S,
    unsafe_include_data_inplace: bool,
) -> Result<(), ()> {
    const SIZE: usize =
        MAX_SUPPORTED_PARTIAL_DIRTY_BUF_SIZE + size_of::<ResidentObjectMetadataBackup>();
    let mut buffer = [0u8; SIZE];

    // step 1: copy metadata backup into buffer
    let base_ptr = (&mut buffer[0]) as *mut u8;

    let (
        origin_slice,
        dirty_buf_backup_slice,
        dirty_buf_bytes,
        dirty_buf_backup_start_ptr,
        dest_offset,
        data_range_start,
        data_range_len
    ) = {
        // important: use this reference of metadata only in this scope
        let metadata = metadata.as_mut().unwrap();

        let metadata_backup_ptr = unsafe { base_ptr.add(MAX_SUPPORTED_PARTIAL_DIRTY_BUF_SIZE) }
            as *mut ResidentObjectMetadataBackup;

        let metadata_backup =
            ResidentObjectMetadataBackup::from_metadata(metadata, next_object_offset);
        unsafe { metadata_backup_ptr.write_unaligned(metadata_backup) };

        // step 2: copy partial dirtiness tracking buffer
        // this will do nothing if partial dirtiness tracking is not enabled
        let dirty_buf_bytes = metadata.inner.partial_dirtiness_tracking_info.byte_count as usize;
        assert!(dirty_buf_bytes <= MAX_SUPPORTED_PARTIAL_DIRTY_BUF_SIZE);
        let dirty_buf_backup_start_ptr =
            unsafe { (metadata_backup_ptr as *mut u8).sub(dirty_buf_bytes) };
        let dirty_buf_backup_slice = unsafe {
            slice_from_raw_parts_mut(dirty_buf_backup_start_ptr, dirty_buf_bytes)
                .as_mut()
                .unwrap()
        };

        let data_range = metadata.dynamic_metadata_to_data_range_mut();
        let data_range_len = data_range.len();
        let data_range = (data_range as *mut [u8]) as *mut u8;

        let origin_slice = metadata
            .inner
            .partial_dirtiness_tracking_info
            .get_dirty_buf_slice(metadata);

        (
            origin_slice,
            dirty_buf_backup_slice,
            dirty_buf_bytes,
            dirty_buf_backup_start_ptr,
            metadata.inner.offset,
            data_range,
            data_range_len
        )

        // metadata reference is dropped here
    };

    let origin_slice = origin_slice.as_ref();

    dirty_buf_backup_slice.copy_from_slice(&origin_slice);

    // step 3: figure out which slice to write
    let res_slice_length = dirty_buf_bytes + size_of::<ResidentObjectMetadataBackup>();

    let slice = unsafe {
        slice_from_raw_parts(dirty_buf_backup_start_ptr, res_slice_length)
            .as_ref()
            .unwrap()
    };

    let dest_offset = dest_offset - dirty_buf_bytes;

    if !unsafe_include_data_inplace {
        storage.write(dest_offset, slice)
    } else {
        // step 4: extra step to combine the metadata and the user data into one storage write call
        // to do that we copy the metadata backup to the ram buffer right before the data
        // !this will destroy data!

        let start_ptr = data_range_start.sub(slice.len());
        let new_slice = slice_from_raw_parts_mut(start_ptr, slice.len() + data_range_len).as_mut().unwrap();

        for i in 0..slice.len() {
            new_slice[i] = slice[i];
        }

        storage.write(dest_offset, new_slice)
    }
}

/*

        let offset = self.inner.offset + calc_backup_obj_user_data_offset();

        if !self.inner.status.is_partial_dirtiness_tracking_enabled() {
            // sync whole object
            let data_range = self.dynamic_metadata_to_data_range();
            storage.write(offset, data_range)?;

            debug_assert_eq!(data_range.len(), self.inner.layout.size());
            Ok(data_range.len())
        } else {
            // sync object partially
            let mut wrapper = self.inner.partial_dirtiness_tracking_info.get_wrapper(self);
            let mut iter = wrapper.dirty_iter();
            let mut synced_byte_count = 0;

            while let Some(range) = iter.next() {
                // persist this data range now
                // this is efficient as the iter will always group together slices of dirty data

                // however, this could still be improved by specifying the initial cost to trigger a write request
                // so that the iterator returns a data range containing even with slices that are already persisted
                // example: [BIG DIRTY][SMALL PERSISTED][BIG DIRTY], if initial cost > SMALL PERSISTED
                // its worth to persist the whole data range with one write call

                let data_range = self.dynamic_metadata_to_data_range();
                let slice = &data_range[range.clone()];
                storage.write(offset + range.start, slice)?;

                synced_byte_count += slice.len()
            }
            Ok(synced_byte_count)
        } */

/// Restores metadata including dirty bit list (if partial dirtiness tracking is enabled)
///
/// **Safety**:
/// - A valid metadata object has to be stored at `offset`
/// - Enough space has to be allocated before `dest_ptr` to fit the partial dirtiness tacking buffer
#[inline]
pub(crate) unsafe fn restore_metadata<S: PersistentStorageModule>(
    offset: usize,
    dest_ptr: *mut ResidentObjectMetadata,
    next_resident_object: *mut ResidentObjectMetadata,
    storage: &mut S,
) -> Result<(), ()> {
    // step 1: read the whole resident object metadata backup and save it
    let backup: ResidentObjectMetadataBackup = read_storage_data(storage, offset)?;

    let metadata = backup.to_metadata(offset, next_resident_object);
    dest_ptr.write(metadata);

    let dest_ref = dest_ptr.as_mut().unwrap();

    // step 2: restore partial dirtiness tracking buffer (if it exists)
    if dest_ref
        .inner
        .status
        .is_partial_dirtiness_tracking_enabled()
    {
        let byte_cnt = dest_ref.inner.partial_dirtiness_tracking_info.byte_count as usize;

        let dest_slice = dest_ref
            .inner
            .partial_dirtiness_tracking_info
            .get_dirty_buf_slice(dest_ptr);
        storage.read(dest_ref.inner.offset - byte_cnt, dest_slice)?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::mem::size_of;

    use super::{calc_backup_obj_layout_static, calc_backup_obj_user_data_offset};

    #[test]
    fn test_backup_obj_layout() {
        test_backup_obj_layout_internal::<usize>();
        test_backup_obj_layout_internal::<u8>();

        struct Test1 {
            _a: usize,
            _b: bool,
            _c: usize,
            _d: bool,
            _e: usize,
        }
        test_backup_obj_layout_internal::<Test1>();

        #[repr(C)]
        struct Test2 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize,
        }
        test_backup_obj_layout_internal::<Test2>();

        #[repr(C, align(64))]
        struct Test3 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize,
        }
        test_backup_obj_layout_internal::<Test3>();

        #[repr(C, align(8))]
        struct Test4 {
            a: usize,
        }
        test_backup_obj_layout_internal::<Test4>();

        #[repr(C, align(16))]
        struct Test5 {
            a: usize,
        }
        test_backup_obj_layout_internal::<Test5>();

        #[repr(C, align(32))]
        struct Test6 {
            a: usize,
        }
        test_backup_obj_layout_internal::<Test6>();

        #[repr(C, align(64))]
        struct Test7 {
            a: usize,
        }
        test_backup_obj_layout_internal::<Test7>();
    }

    fn test_backup_obj_layout_internal<T>() {
        let user_data_offset = calc_backup_obj_user_data_offset();
        let (layout, metadata_offset) = calc_backup_obj_layout_static::<T>(false);

        assert_eq!(user_data_offset + size_of::<T>(), layout.size());
        assert_eq!(metadata_offset, 0);

        let (layout, metadata_offset) = calc_backup_obj_layout_static::<T>(true);

        assert_eq!(
            metadata_offset + user_data_offset + size_of::<T>(),
            layout.size()
        );
    }
}
