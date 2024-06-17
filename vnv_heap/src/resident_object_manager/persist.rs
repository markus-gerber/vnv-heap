use super::{calc_resident_obj_layout, resident_list::SharedResidentListRef, ResidentObjectMetadata, ResidentObjectMetadataBackup, SharedMetadataBackupPtr};
use crate::modules::{allocator::AllocatorModule, persistent_storage::SharedStorageReference};

pub(crate) fn persist(
    resident_list: &SharedResidentListRef,
    backup_list: &SharedMetadataBackupPtr,
    storage_ref: &mut SharedStorageReference,
) {
    // create second storage reference so its easier to do 
    let mut iter = resident_list.iter();
    let mut meta_slot_iter = backup_list.get_atomic_iter();
    while let Some(item) = iter.next() {
        // sync user data if needed
        if item.inner.dirty_status.is_data_dirty() {
            unsafe { item.write_user_data_dynamic(storage_ref).unwrap() };
        }

        // sync metadata if needed
        if item.inner.dirty_status.is_general_metadata_dirty() {
            let backup_slot = if let Some(backup_slot) = item.inner.metadata_backup_node.get() {
                backup_slot
            } else {
                let slot = meta_slot_iter.find(|_, item| {
                    item.is_unused()
                }, storage_ref);

                // slot should exist and no error should occur
                let slot = slot.unwrap().expect("metadata backup slot should exists");
                slot.0.get_data_offset()
            };

            item.write_metadata(storage_ref, backup_slot, true).unwrap();
        }
    }
}

pub(crate) fn restore(
    resident_list: &SharedResidentListRef,
    backup_list: &SharedMetadataBackupPtr,
    storage_ref: &mut SharedStorageReference,
    heap: &mut dyn AllocatorModule,
    resident_buf_base_ptr: *mut u8,
    resident_buf_size: usize
) {
    // step 1: reset resident heaps state
    unsafe {
        heap.reset();
        heap.init(resident_buf_base_ptr, resident_buf_size);
    };

    // step 2: restore metadata + reallocate resident objects
    let mut backup_iter = backup_list.get_atomic_iter();
    while let Some(item) = backup_iter.next(storage_ref).unwrap() {
        if item.1.is_unused() {
            continue;
        }

        let ptr = item.1.resident_ptr as *mut u8;

        // reallocate resident object
        let resident_object_layout = calc_resident_obj_layout(item.1.inner.layout.clone());
        unsafe { heap.allocate_at(resident_object_layout, ptr).unwrap(); }

        let metadata = item.1.to_metadata();

        let remove_original_slot = metadata.inner.metadata_backup_node.get().is_none();

        let ptr = ptr as *mut ResidentObjectMetadata;
        unsafe { ptr.write(metadata) };

        if remove_original_slot {
            // object metadata was not stored before the recovery in this slot
            // so to avoid race conditions, remove it from this slot

            unsafe { ResidentObjectMetadataBackup::make_unused(item.0.get_data_offset(), storage_ref).unwrap(); }
        }
    }

    // step 3: restore resident objects
    let mut meta_iter = resident_list.iter();

    loop {
        let ptr = if let Some(meta_item) = meta_iter.next() {
            (meta_item as *const ResidentObjectMetadata) as *mut ResidentObjectMetadata
        } else {
            break;
        };

        let meta_ref = unsafe { ptr.as_mut().unwrap() };
        unsafe { meta_ref.load_user_data(storage_ref).unwrap() }
    }

}
