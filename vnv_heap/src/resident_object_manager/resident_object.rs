use core::{alloc::Layout, ptr::NonNull, mem::size_of};

use crate::{
    modules::{
        allocator::AllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
    shared_persist_lock::SharedPersistLock,
    util::repr_c_layout,
};

use super::{
    partial_dirtiness_tracking::PartialDirtinessTrackingInfo, resident_list::DeleteHandle, resident_object_metadata::ResidentObjectMetadata
};

/// An object that is currently stored in RAM
///
/// **IMPORTANT**: DO NOT REMOVE `#[repr(C)]` OR REORDER THE
/// FIELDS AS THESE ARE CRUCIAL FOR THIS IMPLEMENTATION!
#[repr(C)]
pub(crate) struct ResidentObject<T: Sized> {
    pub(crate) metadata: ResidentObjectMetadata,
    pub(crate) data: T,
}

impl<T: Sized> ResidentObject<T> {
    /// Unloads this resident object dynamically by indirectly calculating the layout of that object.
    ///
    /// ### Safety
    ///
    /// This is only safe to call if `delete_handle` controls an `ResidentObjectMetadata` that is managed by
    /// `allocator_module` and the `ResidentObjectMetadata` is contained by a `ResidentObject`
    ///
    /// Also there should not be any open references to the `ResidentObject` in any way!
    pub(crate) unsafe fn unload_resident_object<S: PersistentStorageModule, A: AllocatorModule>(
        mut delete_handle: DeleteHandle,
        storage: &mut S,
        allocator_module: &mut SharedPersistLock<*mut A>,
        dirty_size: &mut usize,
        unsafe_no_sync: bool,
        use_partial_dirtiness_tracking: bool
    ) -> Result<(), ()> {
        debug_assert!(
            !delete_handle.get_element().inner.status.is_in_use(),
            "no valid object"
        );

        let ptr = { delete_handle.get_element() as *mut ResidentObjectMetadata };
        let resident_ptr = ResidentObjectMetadata::ptr_to_resident_obj_ptr(ptr);

        let prev_dirty_size = {
            // IMPORTANT: drop reference of resident object again after this block
            let resident_obj: &mut ResidentObject<T> = resident_ptr.as_mut().unwrap();
            let prev_dirty_size = resident_obj.metadata.dirty_size();

            if !unsafe_no_sync {
                // sync unsynced changes
                resident_obj.persist_user_data(storage)?;
            }

            prev_dirty_size
        };

        {
            let (total_layout, obj_offset) = calc_resident_obj_layout_static::<T>(use_partial_dirtiness_tracking);
            let base_ptr = (resident_ptr as *mut u8).sub(obj_offset);

            // IMPORTANT: lock the shared persist lock for this modify block
            // because there are race conditions between this and vnv_persist_all (deallocate is not atomar)

            // unwrap is okay here because there are no other threads concurrently accessing it
            // except from vnv_persist_all, but as it is guaranteed that no other threads run
            // during its execution, it is fine
            let guard = allocator_module.try_lock().unwrap();

            // (for WCET analysis: this is the same/better case as resident_object_metadata_1)

            {
                // IMPORTANT: drop metadata reference at the end of this block
                // remove from resident object list
                let _ = delete_handle.delete();
            }

            // now, as this item is not used anymore, deallocate it
            guard
                .as_mut()
                .unwrap()
                .deallocate(NonNull::new(base_ptr).unwrap(), total_layout);

            drop(guard);
        }

        *dirty_size += prev_dirty_size;

        Ok(())
    }

    /// Persists the user data of this resident object.
    ///
    /// Returns the amount of bytes that are not dirty anymore (these can be used to update the `remaining_dirty_size`).
    ///
    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    pub(crate) unsafe fn persist_user_data<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<usize, ()> {
        if !self.metadata.inner.status.is_data_dirty() {
            return Ok(0);
        }

        self.metadata.persist_user_data_dynamic(storage)
    }
}

pub(crate) fn calc_resident_obj_partial_dirtiness_buf_layout(data_size: usize) -> Layout {
    let (_, byte_count) = PartialDirtinessTrackingInfo::calc_bit_and_byte_count(data_size);
    
    Layout::from_size_align(byte_count, 1).unwrap()
}

#[inline]
pub(crate) fn calc_resident_obj_layout_static<T>(
    use_partial_dirtiness_tracking: bool,
) -> (Layout, usize) {
    if use_partial_dirtiness_tracking {
        let partial_buf = calc_resident_obj_partial_dirtiness_buf_layout(size_of::<T>());
        let (res_layout, metadata_offset) = partial_buf.extend(Layout::new::<ResidentObject<T>>()).unwrap();

        (res_layout.pad_to_align(), metadata_offset)
    } else {
        (Layout::new::<ResidentObject<T>>(), 0)
    }
}

#[inline]
pub(crate) fn calc_resident_obj_layout_dynamic(
    data_layout: &Layout,
    use_partial_dirtiness_tracking: bool,
) -> (Layout, usize) {
    if use_partial_dirtiness_tracking {
        let (tmp_layout, _) = Layout::new::<ResidentObjectMetadata>().extend(data_layout.clone()).unwrap();

        let partial_buf = calc_resident_obj_partial_dirtiness_buf_layout(data_layout.size());
        let (tmp_layout, metadata_offset) = partial_buf.extend(tmp_layout).unwrap();

        (tmp_layout.pad_to_align(), metadata_offset)
    } else {
        (repr_c_layout(&[Layout::new::<ResidentObjectMetadata>(), data_layout.clone()]).unwrap(), 0)
    }
}


#[cfg(test)]
mod test {
    use core::alloc::Layout;

    use crate::resident_object_manager::{
        calc_resident_obj_layout_static,
        resident_object::calc_resident_obj_layout_dynamic,
    };

    #[test]
    fn test_calc_resident_obj_layout() {
        test_calc_resident_obj_layout_internal::<usize>();
        test_calc_resident_obj_layout_internal::<u8>();

        struct Test1 {
            _a: usize,
            _b: bool,
            _c: usize,
            _d: bool,
            _e: usize,
        }
        test_calc_resident_obj_layout_internal::<Test1>();

        #[repr(C)]
        struct Test2 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize,
        }
        test_calc_resident_obj_layout_internal::<Test2>();

        #[repr(C, align(64))]
        struct Test3 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize,
        }
        test_calc_resident_obj_layout_internal::<Test3>();

        #[repr(C, align(8))]
        struct Test4 {
            a: usize,
        }
        test_calc_resident_obj_layout_internal::<Test4>();

        #[repr(C, align(16))]
        struct Test5 {
            a: usize,
        }
        test_calc_resident_obj_layout_internal::<Test5>();

        #[repr(C, align(32))]
        struct Test6 {
            a: usize,
        }
        test_calc_resident_obj_layout_internal::<Test6>();

        #[repr(C, align(64))]
        struct Test7 {
            a: usize,
        }
        test_calc_resident_obj_layout_internal::<Test7>();

    }

    fn test_calc_resident_obj_layout_internal<T: Sized>() {
        let layout = Layout::new::<T>();
        assert_eq!(
            calc_resident_obj_layout_static::<T>(false),
            calc_resident_obj_layout_dynamic(&layout, false)
        );
        assert_eq!(
            calc_resident_obj_layout_static::<T>(true),
            calc_resident_obj_layout_dynamic(&layout, true)
        );
    }
}
