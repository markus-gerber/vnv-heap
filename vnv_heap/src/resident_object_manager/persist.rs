use std::{
    mem::size_of,
    ptr::{copy, null_mut, slice_from_raw_parts},
};

use super::{calc_resident_obj_layout_dynamic, resident_list::SharedResidentListRef, ResidentObjectMetadata, ResidentObjectMetadataBackup};
use crate::modules::{
    allocator::AllocatorModule,
    persistent_storage::{
        persistent_storage_util::{read_storage_data, write_storage_data}, PersistentStorageModule, SharedStorageReference
    },
};

// TODO does currently not work for partial dirtiness tracking
pub(crate) fn persist(
    resident_list: &SharedResidentListRef,
    storage_ref: &mut SharedStorageReference,
) {
    // step 1: get first item of list
    let head = resident_list.get_head();
    let head = unsafe { head.as_ref().unwrap() };

    // step 2: persist all objects
    let mut curr = unsafe { head.as_ptr().read() };

    if curr.is_null() {
        // no objects to be persisted
        let slice_size = size_of::<usize>() as usize;
        write_storage_data(storage_ref, 0, &slice_size).unwrap();
        return;
    }

    let mut slice_end_ptr = (curr as *mut ResidentObjectMetadataBackup) as *mut u8;
    let slice_base_ptr: *mut u8 = unsafe { slice_end_ptr.sub(size_of::<usize>()) };
    while !curr.is_null() {
        let (next, is_data_dirty, backup_obj, data_range, data_range_len) =
            if let Some(item) = unsafe { curr.as_ref() } {
                debug_assert!(
                    !item.inner.status.is_partial_dirtiness_tracking_enabled(),
                    "not implemented"
                );

                let next = unsafe { item.next_resident_object.as_ptr().read() };

                let backup = ResidentObjectMetadataBackup::from_metadata(item);
                let is_data_dirty = item.inner.status.is_data_dirty();
                let data_range = unsafe { item.dynamic_metadata_to_data_range() };

                (
                    next,
                    is_data_dirty,
                    backup,
                    (data_range as *const [u8]) as *mut u8,
                    data_range.len(),
                )
            } else {
                break;
            };

        // drop reference to metadata obj now to perform data surgery

        unsafe { (slice_end_ptr as *mut ResidentObjectMetadataBackup).write(backup_obj) };

        slice_end_ptr = unsafe { slice_end_ptr.add(size_of::<ResidentObjectMetadataBackup>()) };

        if is_data_dirty {
            unsafe {
                // data may be overlapping
                copy(data_range, slice_end_ptr, data_range_len);
                slice_end_ptr = slice_end_ptr.add(data_range_len);
            }
        }

        curr = next;
    }

    // step 3: write the slice length to the start of the slice. This is not optional
    let slice_len = (slice_end_ptr as usize) - (slice_base_ptr as usize);
    unsafe { (slice_base_ptr as *mut usize).write(slice_len) };

    // step 4: write the data slice to storage in one single write call
    let slice = unsafe {
        let tmp = slice_from_raw_parts(
            slice_base_ptr,
            slice_len,
        );
        tmp.as_ref().unwrap()
    };

    storage_ref.write(0, &slice).unwrap();
}

pub(crate) fn restore(
    storage_ref: &mut SharedStorageReference,
    heap: &mut dyn AllocatorModule,
    resident_buf_base_ptr: *mut u8,
    resident_buf_size: usize,
) {
    // step 1: reset resident heaps state
    unsafe {
        heap.reset();
        heap.init(resident_buf_base_ptr, resident_buf_size);
    };

    // step 2: read slice size    
    let slice_size: usize = unsafe { read_storage_data(storage_ref, 0).unwrap() };

    if slice_size == size_of::<usize>() {
        // no object were resident
        return;
    }

    // step 3: read one metadata backup at a time to restore the heap without overwriting any data
    // this is inefficient and could probably be fixed by using a deterministic heap...
    {
        let mut curr_offset = size_of::<usize>();
        while curr_offset < slice_size {
            let backup: ResidentObjectMetadataBackup = unsafe { read_storage_data(storage_ref, curr_offset).unwrap() };
            let ram_offset = backup.ram_offset;

            let metadata = backup.to_metadata(null_mut());
            
            let (total_layout, _) = calc_resident_obj_layout_dynamic(
                &metadata.inner.layout,
                metadata.inner.status.is_partial_dirtiness_tracking_enabled(),
            );
            debug_assert!(ram_offset >= resident_buf_base_ptr as usize);
            debug_assert!(ram_offset + total_layout.size() < (resident_buf_base_ptr as usize) + resident_buf_size);
            unsafe {
                heap.allocate_at(total_layout, ram_offset as *mut u8).unwrap();
            }
            curr_offset += size_of::<ResidentObjectMetadataBackup>();
            if metadata.inner.status.is_data_dirty() {
                curr_offset += metadata.inner.layout.size();
            }
        }
    }

    // step 4: restore metadata and user data at the right location
    let mut curr_offset = size_of::<usize>();
    let mut prev: *mut ResidentObjectMetadata = null_mut();
    while curr_offset < slice_size {
        let backup: ResidentObjectMetadataBackup = unsafe { read_storage_data(storage_ref, curr_offset).unwrap() };
        curr_offset += size_of::<ResidentObjectMetadataBackup>();

        let ram_offset = backup.ram_offset;
        let ram_ptr = (ram_offset as *mut u8) as *mut ResidentObjectMetadata;

        let metadata = backup.to_metadata(null_mut());
        let data_status = metadata.inner.status;
        let data_layout = metadata.inner.layout;
        debug_assert!(!metadata.inner.status.is_partial_dirtiness_tracking_enabled(), "not implemented");

        unsafe { ram_ptr.write(metadata) };

        if let Some(prev) = unsafe { prev.as_ref() } {
            // if there was a previous object, we need to update its "next" ptr
            let ptr = prev.next_resident_object.as_ptr();

            unsafe { ptr.write(ram_ptr) };
        };

        prev = ram_ptr;

        let mut_ref = unsafe { ram_ptr.as_mut().unwrap() };

        // restore data
        if data_status.is_data_dirty() {
            // data is dirty and was stored right next to backup metadata
            let data_dest = unsafe { mut_ref.dynamic_metadata_to_data_range_mut() };

            // slices could overlap
            storage_ref.read(curr_offset, data_dest).unwrap();

            curr_offset += data_layout.size();
        } else {
            // data is not dirty and is stored at its default storage location
            unsafe { mut_ref.load_user_data(storage_ref).unwrap() };
        }
    }

}
