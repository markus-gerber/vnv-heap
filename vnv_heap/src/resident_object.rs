use std::{cell::RefCell, mem::ManuallyDrop, ptr::null_mut};

pub(crate) struct ResidentObjectIdentifier {
    inner: RefCell<*mut ResidentObjectMetadata>
}

impl ResidentObjectIdentifier {
    pub(crate) fn none() -> Self {
        Self {
            inner: RefCell::new(null_mut())
        }
    }
}

#[repr(C)]
pub(crate) struct ResidentObject<T: Sized> {
//    metadata: ResidentObjectMetadata,
    data: T,
}

pub(crate) struct ResidentObjectMetadata {
    is_dirty: bool,

    object_resident_field: *const ResidentObjectIdentifier,

    ref_cnt: usize,

    next_resident_object: *mut ResidentObjectMetadata,
    next_dirty_object: *mut ResidentObjectMetadata,
}

impl ResidentObjectMetadata {

    pub(crate) fn get_next_resident_item(data: &mut ResidentObjectMetadata) -> &mut *mut ResidentObjectMetadata {
        &mut data.next_resident_object
    }

    pub(crate) fn get_next_dirty_item(data: &mut ResidentObjectMetadata) -> &mut *mut ResidentObjectMetadata {
        &mut data.next_dirty_object
    }
}

struct TestMetadataSize {
    is_dirty: bool,
    is_metadata_dirty: bool,

}