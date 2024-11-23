use std::ptr::null_mut;

use super::{
    calc_resident_obj_layout_dynamic, resident_list::SharedResidentListRef,
    resident_object_metadata::ResidentObjectMetadata, ResidentObjectMetadataBackup,
};
use crate::modules::{
    allocator::AllocatorModule,
    persistent_storage::{
        persistent_storage_util::{read_storage_data, write_storage_data},
        SharedStorageReference,
    },
};

pub(crate) fn persist(
    resident_list: &SharedResidentListRef,
    storage_ref: &mut SharedStorageReference,
) {
    // step 1: get iterator to resident list
    let mut iter = resident_list.iter();

    // step 2: persist all objects
    let mut first_item_offset: usize = 0;
    let mut curr = iter.next();
    while let Some(item) = curr {
        if first_item_offset == 0 {
            first_item_offset = item.inner.offset;
        }

        // sync user data if needed
        if item.inner.dirty_status.is_data_dirty() {
            unsafe { item.write_user_data_dynamic(storage_ref).unwrap() };
        }

        let next = iter.next();
        let next_metadata_offset = if let Some(next_item) = next {
            // we didn't reach the end of the list
            // we want to write this offset as the next pointer for the current item
            next_item.inner.offset
        } else {
            // end of list reached, we want to write 0 as the last ptr
            0
        };

        if item.inner.dirty_status.is_general_metadata_dirty() {
            // general metadata is dirty
            // write whole metadata including next pointer to non volatile storage
            item.write_metadata(
                storage_ref,
                item.inner.dirty_status.is_general_metadata_dirty(),
                next_metadata_offset,
            )
            .unwrap();
        } else {
            // general metadata is not dirty
            // however we still need to persist the next pointer
            item.write_next_ptr(
                storage_ref,
                next_metadata_offset
            ).unwrap()
        }

        curr = next;
    }

    // step 3: write the offset of the first resident object to the start of the non volatile storage
    write_storage_data(storage_ref, 0, &first_item_offset).unwrap();
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

    // step 2: get the first storage offset where a resident object is stored
    let mut curr_offset: usize = unsafe { read_storage_data(storage_ref, 0).unwrap() };

    // step 3: reallocate + restore resident objects
    let mut drag_item: Option<&mut ResidentObjectMetadata> = None;
    while curr_offset != 0 {
        let item: ResidentObjectMetadataBackup =
            unsafe { read_storage_data(storage_ref, curr_offset).unwrap() };

        // location where this object should be allocated
        let dest_ptr = item.inner.offset as *mut u8;

        // get layout of resident object
        let resident_object_layout = calc_resident_obj_layout_dynamic(&item.inner.layout);
        unsafe {
            heap.allocate_at(resident_object_layout, dest_ptr).unwrap();
        }
        // location of the next item stored
        let next_offset = item.next_resident_object;

        // convert to resident object metadata and write to RAM
        let metadata = item.to_metadata(curr_offset, null_mut());

        let dest_ptr = dest_ptr as *mut ResidentObjectMetadata;
        unsafe { dest_ptr.write(metadata) };

        // load user data
        let meta_ref = unsafe { dest_ptr.as_mut().unwrap() };
        unsafe { meta_ref.load_user_data(storage_ref).unwrap() }

        // update next ptr of previous list item
        if let Some(drag) = drag_item {
            drag.next_resident_object
                .store(dest_ptr, std::sync::atomic::Ordering::SeqCst);
        }

        drag_item = Some(meta_ref);
        curr_offset = next_offset;
    }
}
