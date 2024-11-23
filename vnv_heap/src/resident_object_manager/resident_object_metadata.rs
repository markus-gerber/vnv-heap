use core::{
    alloc::Layout,
    mem::size_of,
    ptr::{null_mut, slice_from_raw_parts, slice_from_raw_parts_mut, NonNull},
    sync::atomic::AtomicPtr,
};

use memoffset::offset_of;

use crate::{
    modules::{
        allocator::AllocatorModule,
        persistent_storage::{
            persistent_storage_util::write_storage_data, PersistentStorageModule,
        },
    },
    resident_object_manager::calc_resident_obj_layout_dynamic,
};

use super::{
    calc_user_data_offset_dynamic, dirty_status::DirtyStatus, resident_list::DeleteHandle, ResidentObject, ResidentObjectMetadataBackup, SharedPersistLock
};

const SYNCED_METADATA_DIRTY_SIZE: usize = {
    const fn size<T>(_: *const T) -> usize {
        size_of::<T>()
    }

    let obj = ResidentObjectMetadataBackup::new_unused();

    // ref cnt and next_resident_object are not saved
    size(&obj.inner.ref_cnt) + size(&obj.next_resident_object)
};

const UNSYNCED_METADATA_DIRTY_SIZE: usize = size_of::<ResidentObjectMetadataBackup>();

pub(crate) struct ResidentObjectMetadata {
    /// Actual metadata
    pub(crate) inner: ResidentObjectMetadataInner,

    /// Next item in the resident object list
    pub(crate) next_resident_object: AtomicPtr<ResidentObjectMetadata>,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentObjectMetadataInner {
    /// What parts of this resident object are currently dirty?
    pub(crate) dirty_status: DirtyStatus,

    /// Counts the amount of references that are currently held
    /// be the program
    pub(crate) ref_cnt: usize,

    pub(crate) offset: usize,

    pub(crate) layout: Layout,

    /// Used to test that `dynamic_metadata_to_data_range` is correct
    ///
    /// Use `usize::MAX` to disable. This is used when the state will
    /// be restored after a PFI, because VNVHeap has no idea what type
    /// belongs to which metadata.
    #[cfg(debug_assertions)]
    pub(super) data_offset: usize,
}

impl ResidentObjectMetadataInner {
    pub(super) fn new<T: Sized>(offset: usize) -> Self {
        ResidentObjectMetadataInner {
            dirty_status: DirtyStatus::new_metadata_dirty(),
            ref_cnt: 0,
            layout: Layout::new::<T>(),
            offset,

            #[cfg(debug_assertions)]
            data_offset: offset_of!(ResidentObject<T>, data),
        }
    }
}

impl Default for ResidentObjectMetadataInner {
    fn default() -> Self {
        Self {
            dirty_status: Default::default(),
            ref_cnt: Default::default(),
            offset: Default::default(),
            layout: Layout::new::<()>(),
            #[cfg(debug_assertions)]
            data_offset: usize::MAX,
        }
    }
}

impl ResidentObjectMetadata {
    pub(super) fn new<T: Sized>(offset: usize) -> Self {
        ResidentObjectMetadata {
            next_resident_object: AtomicPtr::new(null_mut()),
            inner: ResidentObjectMetadataInner::new::<T>(offset),
        }
    }

    pub(crate) const fn fresh_object_dirty_size() -> usize {
        UNSYNCED_METADATA_DIRTY_SIZE
    }

    /// Change in dirty size for `metadata_dirty=false` -> `metadata_dirty=true`
    pub(crate) const fn metadata_dirty_transition_size() -> usize {
        UNSYNCED_METADATA_DIRTY_SIZE - SYNCED_METADATA_DIRTY_SIZE
    }

    #[inline]
    pub(crate) unsafe fn to_resident_obj_ptr<T>(&mut self) -> *mut ResidentObject<T> {
        ResidentObjectMetadata::ptr_to_resident_obj_ptr(self)
    }

    #[inline]
    pub(crate) const unsafe fn ptr_to_resident_obj_ptr<T>(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut ResidentObject<T> {
        ptr as *mut ResidentObject<T>
    }

    pub(crate) fn dirty_size(&self) -> usize {
        let mut cnt = 0;
        if self.inner.dirty_status.is_data_dirty() {
            cnt += self.inner.layout.size()
        }

        if self.inner.dirty_status.is_general_metadata_dirty() {
            cnt += UNSYNCED_METADATA_DIRTY_SIZE
        } else {
            cnt += SYNCED_METADATA_DIRTY_SIZE;
        }

        cnt
    }

    unsafe fn dynamic_metadata_to_data_range_internal(&self) -> *const u8 {
        let meta_ptr = ((self as *const ResidentObjectMetadata) as *const u8)
            .add(size_of::<ResidentObjectMetadata>());

        // align base pointer (add alignment, because T could be aligned)
        let base_ptr = ((meta_ptr as usize) + (self.inner.layout.align() - 1))
            & !(self.inner.layout.align() - 1);

        // convert back to pointer
        let base_ptr = base_ptr as *const u8;

        // test if the right offset was applied
        #[cfg(debug_assertions)]
        {
            // check that data offset was not disabled
            if self.inner.data_offset != usize::MAX {
                debug_assert_eq!(
                    ((self as *const ResidentObjectMetadata) as *const u8).add(self.inner.data_offset),
                    base_ptr,
                    "Results in an error if the formula for manually getting the address of the data is wrong"
                );
            }
        }

        base_ptr
    }

    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    #[inline]
    unsafe fn dynamic_metadata_to_data_range(&self) -> &[u8] {
        let base_ptr = self.dynamic_metadata_to_data_range_internal();

        slice_from_raw_parts(base_ptr, self.inner.layout.size())
            .as_ref()
            .unwrap()
    }

    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    #[inline]
    unsafe fn dynamic_metadata_to_data_range_mut(&mut self) -> &mut [u8] {
        let base_ptr = self.dynamic_metadata_to_data_range_internal() as *mut u8;

        slice_from_raw_parts_mut(base_ptr, self.inner.layout.size())
            .as_mut()
            .unwrap()
    }

    pub(crate) unsafe fn load_user_data<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<(), ()> {
        let offset = self.inner.offset + calc_user_data_offset_dynamic(&self.inner.layout);
        let range = self.dynamic_metadata_to_data_range_mut();
        storage.read(offset, range)
    }

    /// Unloads this resident object dynamically by indirectly calculating the layout of that object.
    /// This is a good option to do if you don't know `T` of this resident object.
    ///
    /// If you know `T`, you should use `unload_resident_object` of `ResidentObject` instead.
    ///
    /// ### Safety
    ///
    /// This is only safe to call if `delete_handle` controls an `ResidentObjectMetadata` that is managed by `allocator_module` and
    /// the `ResidentObjectMetadata` is contained by a `ResidentObject`
    pub(crate) unsafe fn unload_resident_object_dynamic<
        S: PersistentStorageModule,
        A: AllocatorModule,
    >(
        mut delete_handle: DeleteHandle,
        storage: &mut S,
        allocator_module: &SharedPersistLock<*mut A>,
        dirty_size: &mut usize,
    ) -> Result<(), ()> {
        debug_assert_eq!(
            delete_handle.get_element().inner.ref_cnt,
            0,
            "no valid object"
        );

        let prev_dirty_size = delete_handle.get_element().dirty_size();

        // sync unsynced changes
        let _ = delete_handle
            .get_element()
            .persist_user_data_dynamic(storage)?;

        {
            // IMPORTANT: lock the shared persist lock for this modify block
            // because there are race conditions between this and vnv_persist_all (deallocate is not atomar)

            // unwrap is okay here because there are no other threads concurrently accessing it
            // except from vnv_persist_all, but as it is guaranteed that no other threads run
            // during its execution, it is fine
            let guard = allocator_module.try_lock().unwrap();

            // remove from resident object list
            let item_ref = delete_handle.delete();

            // now, as this item is not used anymore, deallocate it
            let resident_obj_ptr = unsafe { item_ref.to_resident_obj_ptr::<()>() } as *mut u8;
            let resident_obj_ptr = NonNull::new(resident_obj_ptr).unwrap();

            let resident_obj_layout = calc_resident_obj_layout_dynamic(&item_ref.inner.layout);

            resident_obj_ptr.as_ptr().drop_in_place();
            guard
                .as_mut()
                .unwrap()
                .deallocate(resident_obj_ptr, resident_obj_layout);

            drop(guard);
        }

        *dirty_size += prev_dirty_size;

        Ok(())
    }

    /// Persists the user data of this resident object if you don't know the type `T` of the inner data.
    /// If you know the type `T`, you should use `persist_user_data` of `ResidentObject`instead.
    ///
    /// Returns the amount of bytes that are not dirty anymore (these can be used to update the `remaining_dirty_size`).
    ///
    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    pub(crate) unsafe fn persist_user_data_dynamic<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<usize, ()> {
        if !self.inner.dirty_status.is_data_dirty() {
            return Ok(0);
        }

        self.write_user_data_dynamic(storage)?;

        // everything is persisted, not dirty anymore
        self.inner.dirty_status.set_data_dirty(false);

        if !self.inner.dirty_status.is_general_metadata_dirty() {
            ResidentObjectMetadataBackup::flush_dirty_status(
                self.inner.offset,
                &self.inner.dirty_status,
                storage,
            )
            .unwrap();
        }

        // set this again because of race conditions:
        // if vnv_persist_all is called after first set_data_dirty and before flushing dirty status
        self.inner.dirty_status.set_data_dirty(false);

        Ok(self.inner.layout.size())
    }

    /// Writes the user data of this resident object if you don't know the type `T` of the inner data.
    /// This function differs from `persist_user_data_dynamic` that is does not update the dirty state of this object.
    ///
    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    pub(crate) unsafe fn write_user_data_dynamic<S: PersistentStorageModule>(
        &self,
        storage: &mut S,
    ) -> Result<(), ()> {
        // persist data from dynamic data range (stored layout of T is used for that)
        let data_range = self.dynamic_metadata_to_data_range();
        let offset = self.inner.offset + calc_user_data_offset_dynamic(&self.inner.layout);
        storage.write(offset, data_range)?;
        Ok(())
    }

    /// Persists this metadata and saves it to the `MetadataBackupList`.
    ///
    /// If it was previously saved to it, that slot will be updated, without accessing `backup_list`
    pub(crate) fn persist_metadata<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<usize, ()> {
        if !self.inner.dirty_status.is_general_metadata_dirty() {
            return Ok(0);
        }

        match self.write_metadata(storage, false, 0) {
            Ok(()) => {}
            Err(()) => {
                return Err(());
            }
        };

        self.inner.dirty_status.set_general_metadata_dirty(false);
        Ok(UNSYNCED_METADATA_DIRTY_SIZE - SYNCED_METADATA_DIRTY_SIZE)
    }

    pub(crate) fn write_metadata<S: PersistentStorageModule>(
        &self,
        storage: &mut S,
        metadata_dirty: bool,
        next_item_offset: usize,
    ) -> Result<(), ()> {
        let mut backup_data = ResidentObjectMetadataBackup::from_metadata(self, next_item_offset);
        backup_data
            .inner
            .dirty_status
            .set_general_metadata_dirty(metadata_dirty);
        write_storage_data(storage, self.inner.offset, &backup_data)?;

        Ok(())
    }

    pub(crate) fn write_next_ptr<S: PersistentStorageModule>(
        &self,
        storage: &mut S,
        next_item_offset: usize,
    ) -> Result<(), ()> {
        unsafe {
            ResidentObjectMetadataBackup::write_next_ptr(
                self.inner.offset,
                next_item_offset,
                storage,
            )
        }
    }

}

#[cfg(test)]
mod test {
    use std::mem::size_of;

    use crate::resident_object_manager::{ResidentObject, ResidentObjectMetadata};

    #[test]
    fn test_dynamic_metadata_to_data_range_1() {
        #[derive(Default)]
        struct TestData1 {
            _a: u64,
            _b: bool,
            _c: u16,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData1>();

        #[derive(Default)]
        #[repr(C, align(64))]
        struct TestData2 {
            b: bool,
            c: u16,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData2>();

        #[derive(Default)]
        struct TestData3 {
            _b: bool,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData3>();

        #[derive(Default)]
        struct TestData4 {
            _a: u128,
            _b: u128,
            _c: u128,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData4>();

        #[derive(Default)]
        struct TestData5 {
            _a: u128,
            _b: u128,
            _d: bool,
            _c: u128,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData4>();

        #[derive(Default)]
        #[repr(C, align(16))]
        struct TestData6 {
            b: bool,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData6>();
    }

    fn test_dynamic_metadata_to_data_range_internal<T: Default>() {
        let obj = ResidentObject {
            metadata: ResidentObjectMetadata::new::<T>(0),
            data: T::default(),
        };
        let original_ptr = (&obj.data) as *const T;

        let data_range = unsafe { obj.metadata.dynamic_metadata_to_data_range() };

        assert_eq!(original_ptr as *const u8, data_range.as_ptr());
        assert_eq!(size_of::<T>(), data_range.len());
    }
}
