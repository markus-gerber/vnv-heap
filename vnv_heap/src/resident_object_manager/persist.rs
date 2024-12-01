use core::ptr::null_mut;

use super::{
    calc_resident_obj_layout_dynamic, persist_whole_metadata, resident_list::SharedResidentListRef,
    resident_object_metadata::ResidentObjectMetadata, restore_metadata,
    ResidentObjectMetadataBackup,
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
    let head = resident_list.get_head();
    let head = unsafe { head.as_ref().unwrap() };

    // step 2: persist all objects
    let mut first_item_offset: usize = 0;
    let mut curr = head.load(std::sync::atomic::Ordering::SeqCst);
    while !curr.is_null() {
        let (next, next_metadata_offset, persist_data_later) = if let Some(item) = unsafe { curr.as_ref() } {
            // drop reference now to perform persist surgery
            let mut persist_data_later = false;
            if item.inner.status.is_data_dirty() {
                if item.inner.status.is_partial_dirtiness_tracking_enabled() || cfg!(feature = "enable_general_metadata_runtime_persist") {
                    // TODO persisting data for an object with partial dirtiness tracking can be optimized more
                    // e.g. group data and metadata into one write call
                    unsafe { item.write_user_data_dynamic(storage_ref).unwrap() };

                } else {
                    // we can persist metadata and data together yay!
                    persist_data_later = true;
                }
            }

            if first_item_offset == 0 {
                first_item_offset = item.inner.offset;
            }

            let next = item
                .next_resident_object
                .load(std::sync::atomic::Ordering::SeqCst);
            let next_metadata_offset = if let Some(next_item) = unsafe { next.as_ref() } {
                // we didn't reach the end of the list
                // we want to write this offset as the next pointer for the current item
                next_item.inner.offset
            } else {
                // end of list reached, we want to write 0 as the last ptr
                0
            };

            (next, next_metadata_offset, persist_data_later)
        } else {
            break;
        };

        #[cfg(not(feature = "enable_general_metadata_runtime_persist"))]
        {
            unsafe {
                persist_whole_metadata(
                    curr,
                    next_metadata_offset,
                    storage_ref,
                    persist_data_later,
                )
                .unwrap()
            };    
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

        // get layout of resident object
        let data_layout = item.inner.layout;
        let (total_layout, object_offset) = calc_resident_obj_layout_dynamic(
            &data_layout,
            item.inner.status.is_partial_dirtiness_tracking_enabled(),
        );

        // storage location of the metadata
        let metadata_ptr = (item.inner.offset as *mut u8) as *mut ResidentObjectMetadata;
        // base pointer of the resident object including dirtiness tracking buffer (if enabled)
        let base_ptr = unsafe { (metadata_ptr as *mut u8).sub(object_offset) };

        unsafe {
            heap.allocate_at(total_layout, base_ptr).unwrap();
        }
        // location of the next item stored
        let next_offset = item.next_resident_object;

        let meta_ref = unsafe {
            restore_metadata(curr_offset, metadata_ptr, null_mut(), storage_ref).unwrap();
            metadata_ptr.as_mut().unwrap()
        };

        // load user data
        unsafe { meta_ref.load_user_data(storage_ref).unwrap() };

        // update next ptr of previous list item
        if let Some(drag) = drag_item {
            drag.next_resident_object
                .store(metadata_ptr, std::sync::atomic::Ordering::SeqCst);
        }

        drag_item = Some(meta_ref);
        curr_offset = next_offset;
    }
}
