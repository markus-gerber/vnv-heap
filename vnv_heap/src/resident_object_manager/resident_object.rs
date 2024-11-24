use core::{alloc::Layout, ptr::NonNull};
use memoffset::offset_of;

use crate::{
    modules::{
        allocator::AllocatorModule,
        persistent_storage::{
            persistent_storage_util::write_storage_data, PersistentStorageModule,
        },
    },
    shared_persist_lock::SharedPersistLock,
    util::{repr_c_layout, round_up_to_nearest},
};

use super::{
    resident_list::DeleteHandle, resident_object_metadata::ResidentObjectMetadata,
    ResidentObjectMetadataBackup,
};


/// An object that is currently stored in RAM
///
/// **IMPORTANT**: DO NOT REMOVE `#[repr(C)]` OR REORDER THE
/// FIELDS AS THESE ARE CRUCIAL FOR THIS IMPLEMENTATION!
#[repr(C)]
pub(crate) struct ResidentObject<T: Sized> {
    pub(super) metadata: ResidentObjectMetadata,
    pub(super) data: T,
}

impl<T: Sized> ResidentObject<T> {
    /// Unloads this resident object dynamically by indirectly calculating the layout of that object.
    ///
    /// ### Safety
    ///
    /// This is only safe to call if `delete_handle` controls an `ResidentObjectMetadata` that is managed by `allocator_module` and
    /// the `ResidentObjectMetadata` is contained by a `ResidentObject`
    ///
    /// Also there should not be any open references to the `ResidentObject` in any way!
    pub(crate) unsafe fn unload_resident_object<S: PersistentStorageModule, A: AllocatorModule>(
        mut delete_handle: DeleteHandle,
        storage: &mut S,
        allocator_module: &mut SharedPersistLock<*mut A>,
        dirty_size: &mut usize,
        unsafe_no_sync: bool,
    ) -> Result<(), ()> {
        debug_assert!(
            !delete_handle.get_element().inner.dirty_status.is_in_use(),
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
            // IMPORTANT: lock the shared persist lock for this modify block
            // because there are race conditions between this and vnv_persist_all (deallocate is not atomar)

            // unwrap is okay here because there are no other threads concurrently accessing it
            // except from vnv_persist_all, but as it is guaranteed that no other threads run
            // during its execution, it is fine
            let guard = allocator_module.try_lock().unwrap();

            {
                // IMPORTANT: drop metadata reference at the end of this block
                // remove from resident object list
                let _ = delete_handle.delete();
            }

            // now, as this item is not used anymore, deallocate it
            resident_ptr.drop_in_place();
            let obj_layout = Layout::new::<ResidentObject<T>>();
            guard
                .as_mut()
                .unwrap()
                .deallocate(NonNull::new(resident_ptr as *mut u8).unwrap(), obj_layout);

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
        if !self.metadata.inner.dirty_status.is_data_dirty() {
            return Ok(0);
        }

        let data_offset = calc_user_data_offset_static::<T>();
        write_storage_data(storage, self.metadata.inner.offset + data_offset, &self.data)?;

        // everything is persisted, not dirty anymore
        self.metadata.inner.dirty_status.set_data_dirty(false);

        if !self.metadata.inner.dirty_status.is_general_metadata_dirty() {
            // you have to flush dirty status as well
            ResidentObjectMetadataBackup::flush_dirty_status(
                self.metadata.inner.offset,
                &self.metadata.inner.dirty_status,
                storage,
            )
            .unwrap();
        }

        // set this again because of race conditions:
        // if vnv_persist_all is called after first set_data_dirty and before flushing dirty status
        self.metadata.inner.dirty_status.set_data_dirty(false);

        Ok(self.metadata.inner.layout.size())
    }
}

#[inline]
pub(crate) const fn calc_resident_obj_layout_static<T>() -> Layout {
    Layout::new::<ResidentObject<T>>()
}

#[inline]
pub(crate) fn calc_resident_obj_layout_dynamic(data_layout: &Layout) -> Layout {
    // get layout of ResidentObject
    repr_c_layout(&[Layout::new::<ResidentObjectMetadata>(), data_layout.clone()]).unwrap()
}

#[inline]
pub(crate) fn calc_user_data_offset_static<T>() -> usize {
    offset_of!(ResidentObject<T>, data)
}

#[inline]
pub(crate) fn calc_user_data_offset_dynamic(layout: &Layout) -> usize {
    let base_offset = offset_of!(ResidentObject<()>, data);
    round_up_to_nearest(base_offset, layout.align())
}

#[cfg(test)]
mod test {
    use core::alloc::Layout;

    use memoffset::offset_of;

    use crate::resident_object_manager::{calc_resident_obj_layout_static, calc_user_data_offset_dynamic, resident_object::{
        calc_resident_obj_layout_dynamic, ResidentObject,
    }};

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
    }


    #[test]
    fn test_calc_user_data_offset_dynamic() {
        test_calc_user_data_offset_dynamic_internal::<()>();
        test_calc_user_data_offset_dynamic_internal::<usize>();
        test_calc_user_data_offset_dynamic_internal::<u8>();

        struct Test1 {
            _a: usize,
            _b: bool,
            _c: usize,
            _d: bool,
            _e: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test1>();

        #[repr(C)]
        struct Test2 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test2>();

        #[repr(C, align(64))]
        struct Test3 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test3>();

        #[repr(C, align(8))]
        struct Test4 {
            a: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test4>();

        #[repr(C, align(16))]
        struct Test5 {
            a: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test5>();

        #[repr(C, align(32))]
        struct Test6 {
            a: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test6>();

        #[repr(C, align(64))]
        struct Test7 {
            a: usize,
        }
        test_calc_user_data_offset_dynamic_internal::<Test7>();
    }

    fn test_calc_resident_obj_layout_internal<T: Sized>() {
        let layout = Layout::new::<T>();
        assert_eq!(
            calc_resident_obj_layout_static::<T>(),
            calc_resident_obj_layout_dynamic(&layout)
        );
    }

    fn test_calc_user_data_offset_dynamic_internal<T: Sized>() {
        let layout = Layout::new::<T>();
        assert_eq!(
            offset_of!(ResidentObject<T>, data),
            calc_user_data_offset_dynamic(&layout)
        );
    }
}
