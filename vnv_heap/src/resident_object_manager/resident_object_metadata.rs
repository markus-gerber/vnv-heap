use core::{
    alloc::Layout,
    mem::size_of,
    ptr::{null_mut, slice_from_raw_parts, slice_from_raw_parts_mut, NonNull},
    sync::atomic::AtomicPtr,
};

use memoffset::offset_of;

use crate::{
    modules::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule},
    resident_object_manager::calc_resident_obj_layout_dynamic, util::round_up_to_nearest,
};

#[cfg(feature = "enable_general_metadata_runtime_persist")]
use super::{
    persist_general_metadata, GENERAL_METADATA_BACKUP_SIZE, METADATA_EXCEPT_GENERAL_BACKUP_SIZE
};

use super::{
    calc_backup_obj_user_data_offset, partial_dirtiness_tracking::PartialDirtinessTrackingInfo,
    resident_list::DeleteHandle,
    resident_object_status::ResidentObjectStatus, ResidentObject, SharedPersistLock,
    TOTAL_METADATA_BACKUP_SIZE,
};

const fn calc_dirty_metadata_dirty_byte_cnt(
    enabled_partial_dirtiness_tracking: bool,
    data_size: usize,
) -> usize {
    if !enabled_partial_dirtiness_tracking {
        TOTAL_METADATA_BACKUP_SIZE
    } else {
        let (_, byte_cnt) = PartialDirtinessTrackingInfo::calc_bit_and_byte_count(data_size);
        TOTAL_METADATA_BACKUP_SIZE + byte_cnt
    }
}

#[cfg(feature = "enable_general_metadata_runtime_persist")]
const fn calc_general_metadata_synced_dirty_byte_cnt(
    enabled_partial_dirtiness_tracking: bool,
    data_size: usize,
) -> usize {
    if !enabled_partial_dirtiness_tracking {
        METADATA_EXCEPT_GENERAL_BACKUP_SIZE
    } else {
        let (_, byte_cnt) = PartialDirtinessTrackingInfo::calc_bit_and_byte_count(data_size);
        METADATA_EXCEPT_GENERAL_BACKUP_SIZE + byte_cnt
    }
}

pub(crate) struct ResidentObjectMetadata {
    /// Actual metadata
    pub(crate) inner: ResidentObjectMetadataInner,

    /// Next item in the resident object list
    pub(crate) next_resident_object: AtomicPtr<ResidentObjectMetadata>,
}

#[derive(Clone, Copy)]
pub(crate) struct ResidentObjectMetadataInner {
    /// What parts of this resident object are currently dirty?
    pub(crate) status: ResidentObjectStatus,

    /// Gives a nice interface for accessing partial dirtiness data
    /// This data will not be persisted (as it is reconstructible from `status`).
    pub(crate) partial_dirtiness_tracking_info: PartialDirtinessTrackingInfo,

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
    pub(super) fn new<T: Sized>(offset: usize, partial_dirtiness_tracking: bool) -> Self {
        let partial_dirtiness_tracking_info = if partial_dirtiness_tracking {
            PartialDirtinessTrackingInfo::new_used::<T>()
        } else {
            PartialDirtinessTrackingInfo::new_unused()
        };

        ResidentObjectMetadataInner {
            status: ResidentObjectStatus::new_metadata_dirty(partial_dirtiness_tracking),
            layout: Layout::new::<T>(),
            offset,
            partial_dirtiness_tracking_info,

            #[cfg(debug_assertions)]
            data_offset: offset_of!(ResidentObject<T>, data),
        }
    }
}

impl Default for ResidentObjectMetadataInner {
    fn default() -> Self {
        Self {
            status: Default::default(),
            offset: Default::default(),
            layout: Layout::new::<()>(),
            partial_dirtiness_tracking_info: PartialDirtinessTrackingInfo::new_unused(),

            #[cfg(debug_assertions)]
            data_offset: usize::MAX,
        }
    }
}

impl ResidentObjectMetadata {
    pub(crate) fn new<T: Sized>(offset: usize, partial_dirtiness_tracking: bool) -> Self {
        ResidentObjectMetadata {
            next_resident_object: AtomicPtr::new(null_mut()),
            inner: ResidentObjectMetadataInner::new::<T>(offset, partial_dirtiness_tracking),
        }
    }

    pub(crate) const fn fresh_object_dirty_size<T>(
        enable_partial_dirtiness_tracking: bool,
    ) -> usize {
        calc_dirty_metadata_dirty_byte_cnt(enable_partial_dirtiness_tracking, size_of::<T>())
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
        if self.inner.status.is_data_dirty() {
            if !self.inner.status.is_partial_dirtiness_tracking_enabled() {
                // whole object is dirty
                cnt += self.inner.layout.size();
            } else {
                let wrapper =
                    unsafe { self.inner.partial_dirtiness_tracking_info.get_wrapper(self) };
                cnt += wrapper.get_dirty_size();
            }
        }

        #[cfg(feature = "enable_general_metadata_runtime_persist")]
        {
            if self.inner.status.is_general_metadata_dirty() {
                cnt += calc_dirty_metadata_dirty_byte_cnt(
                    self.inner.status.is_partial_dirtiness_tracking_enabled(),
                    self.inner.layout.size(),
                );
            } else {
                cnt += calc_general_metadata_synced_dirty_byte_cnt(
                    self.inner.status.is_partial_dirtiness_tracking_enabled(),
                    self.inner.layout.size(),
                );
            }
        }

        #[cfg(not(feature = "enable_general_metadata_runtime_persist"))]
        {
            cnt += calc_dirty_metadata_dirty_byte_cnt(
                self.inner.status.is_partial_dirtiness_tracking_enabled(),
                self.inner.layout.size(),
            );
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
            // we have to check it that way because of race conditions with the persist and restore routine
            // (its not perfect that way, but at least better, then changing the ordering of the two conditions)
            if ((self as *const ResidentObjectMetadata) as *const u8).add(self.inner.data_offset) != base_ptr {
                if self.inner.data_offset != usize::MAX {
                    debug_assert!(
                        false,
                        "{} != {}. Results in an error if the formula for manually getting the address of the data is wrong (data_offset: {} vs {})",
                        ((self as *const ResidentObjectMetadata) as *const u8).add(self.inner.data_offset) as usize,
                        base_ptr as usize,
                        self.inner.data_offset,
                        round_up_to_nearest(size_of::<ResidentObjectMetadata>(), self.inner.layout.align())
                    );
                }
            }
        }

        base_ptr
    }

    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    #[inline]
    pub(crate) unsafe fn dynamic_metadata_to_data_range(&self) -> &[u8] {
        let base_ptr = self.dynamic_metadata_to_data_range_internal();

        slice_from_raw_parts(base_ptr, self.inner.layout.size())
            .as_ref()
            .unwrap()
    }

    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    #[inline]
    pub(crate) unsafe fn dynamic_metadata_to_data_range_mut(&mut self) -> &mut [u8] {
        let base_ptr = self.dynamic_metadata_to_data_range_internal() as *mut u8;

        slice_from_raw_parts_mut(base_ptr, self.inner.layout.size())
            .as_mut()
            .unwrap()
    }

    pub(crate) unsafe fn load_user_data<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<(), ()> {
        let offset = self.inner.offset + calc_backup_obj_user_data_offset();
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
        debug_assert!(
            !delete_handle.get_element().inner.status.is_in_use(),
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

            let (total_layout, obj_offset) = calc_resident_obj_layout_dynamic(
                &item_ref.inner.layout,
                item_ref
                    .inner
                    .status
                    .is_partial_dirtiness_tracking_enabled(),
            );

            // now, as this item is not used anymore, deallocate it
            let resident_obj_ptr = unsafe { item_ref.to_resident_obj_ptr::<()>() } as *mut u8;

            let base_ptr = resident_obj_ptr.sub(obj_offset);
            let base_ptr = NonNull::new(base_ptr).unwrap();

            guard.as_mut().unwrap().deallocate(base_ptr, total_layout);

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
        if !self.inner.status.is_data_dirty() {
            return Ok(0);
        }

        let size_persisted = self.write_user_data_dynamic(storage)?;

        // everything is persisted, not dirty anymore
        self.inner.status.set_data_dirty(false);

        // does nothing if partial dirtiness tracking is not enabled
        self.inner
            .partial_dirtiness_tracking_info
            .get_wrapper(self)
            .set_all_blocks_synced();

        Ok(size_persisted)
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
    ) -> Result<usize, ()> {
        // persist data from dynamic data range (stored layout of T is used for that)

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
        }
    }

    /// Persists the general metadata.
    #[cfg(feature = "enable_general_metadata_runtime_persist")]
    pub(crate) fn persist_general_metadata<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<usize, ()> {
        if !self.inner.status.is_general_metadata_dirty() {
            return Ok(0);
        }

        persist_general_metadata(self, storage)?;

        self.inner.status.set_general_metadata_dirty(false);
        Ok(GENERAL_METADATA_BACKUP_SIZE)
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
            metadata: ResidentObjectMetadata::new::<T>(0, false),
            data: T::default(),
        };
        let original_ptr = (&obj.data) as *const T;

        let data_range = unsafe { obj.metadata.dynamic_metadata_to_data_range() };

        assert_eq!(original_ptr as *const u8, data_range.as_ptr());
        assert_eq!(size_of::<T>(), data_range.len());
    }
}
