use core::{
    alloc::Layout,
    mem::size_of,
    ptr::{null_mut, slice_from_raw_parts},
    sync::atomic::AtomicPtr,
};
use std::ptr::{slice_from_raw_parts_mut, NonNull};

use memoffset::offset_of;

use crate::{
    modules::{
        allocator::AllocatorModule,
        persistent_storage::{
            persistent_storage_util::write_storage_data, PersistentStorageModule,
        },
    },
    shared_persist_lock::SharedPersistLock,
    util::repr_c_layout,
};

use super::{
    dirty_status::DirtyStatus, metadata_backup_info::MetadataBackupInfo,
    metadata_backup_list::MetadataBackupList, resident_list::DeleteHandle,
    ResidentObjectMetadataBackup,
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
    ) -> Result<(), ()> {
        debug_assert_eq!(
            delete_handle.get_element().inner.ref_cnt,
            0,
            "no valid object"
        );

        let ptr = { delete_handle.get_element() as *mut ResidentObjectMetadata };
        let resident_ptr = ResidentObjectMetadata::ptr_to_resident_obj_ptr(ptr);

        let prev_dirty_size = {
            // IMPORTANT: drop reference of resident object again after this block
            let resident_obj: &mut ResidentObject<T> = resident_ptr.as_mut().unwrap();
            let prev_dirty_size = resident_obj.metadata.dirty_size();

            // sync unsynced changes
            resident_obj.persist_user_data(storage)?;

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
                let item_ref = delete_handle.delete();

                // remove metadata from backup list
                item_ref.remove_metadata_backup(storage)?;
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

        write_storage_data(storage, self.metadata.inner.offset, &self.data)?;

        // everything is persisted, not dirty anymore
        self.metadata.inner.dirty_status.set_data_dirty(false);

        if !self.metadata.inner.dirty_status.is_general_metadata_dirty() {
            // you have to flush dirty status as well

            // because metadata is not dirty, there has to be a backup slot
            let backup_slot = self.metadata.inner.metadata_backup_node.get().unwrap();

            ResidentObjectMetadataBackup::flush_dirty_status(backup_slot, &self.metadata.inner.dirty_status, storage).unwrap();
        }

        // set this again because of race conditions:
        // if vnv_persist_all is called after first set_data_dirty and before flushing dirty status
        self.metadata.inner.dirty_status.set_data_dirty(false);

        Ok(self.metadata.inner.layout.size())
    }
}

#[inline]
pub(crate) fn calc_resident_obj_layout(data_layout: Layout) -> Layout {
    // get layout of ResidentObject
    repr_c_layout(&[Layout::new::<ResidentObjectMetadata>(), data_layout.clone()]).unwrap()
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
    pub(crate) dirty_status: DirtyStatus,

    /// Counts the amount of references that are currently held
    /// be the program
    pub(crate) ref_cnt: usize,

    pub(crate) offset: usize,

    pub(crate) layout: Layout,

    /// Offset of the metadata backup node that is being used.
    pub(crate) metadata_backup_node: MetadataBackupInfo,

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
            metadata_backup_node: MetadataBackupInfo::empty(),

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
            metadata_backup_node: MetadataBackupInfo::empty(),
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

    /// The same as `ptr_to_resident_obj_ptr` but without type `T`
    #[inline]
    pub(super) unsafe fn ptr_to_resident_obj_ptr_base(ptr: *mut ResidentObjectMetadata) -> *mut u8 {
        ptr as *mut u8
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
        let offset = self.inner.offset;
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

            // remove metadata from backup list
            item_ref.remove_metadata_backup(storage)?;

            // now, as this item is not used anymore, deallocate it
            let resident_obj_ptr = unsafe { item_ref.to_resident_obj_ptr::<()>() } as *mut u8;
            let resident_obj_ptr = NonNull::new(resident_obj_ptr).unwrap();

            let resident_obj_layout = calc_resident_obj_layout(item_ref.inner.layout);

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
            // you have to flush dirty status as well

            // because metadata is not dirty, there has to be a backup slot
            let backup_slot = self.inner.metadata_backup_node.get().unwrap();

            ResidentObjectMetadataBackup::flush_dirty_status(backup_slot, &self.inner.dirty_status, storage).unwrap();
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
        storage.write(self.inner.offset, data_range)?;
        Ok(())
    }

    /// Persists this metadata and saves it to the `MetadataBackupList`.
    ///
    /// If it was previously saved to it, that slot will be updated, without accessing `backup_list`
    pub(crate) fn persist_metadata<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
        backup_list: &MetadataBackupList,
    ) -> Result<usize, ()> {
        if !self.inner.dirty_status.is_general_metadata_dirty() {
            return Ok(0);
        }

        if let Some(backup_slot) = self.inner.metadata_backup_node.get() {
            // save stuff to this backup slot
            match self.write_metadata(storage, backup_slot, false) {
                Ok(()) => {}
                Err(()) => {
                    // metadata could not be saved, reset this slot
                    let backup_data = ResidentObjectMetadataBackup::new_unused();

                    // this should not fail
                    write_storage_data(storage, backup_slot, &backup_data).unwrap();

                    self.inner.metadata_backup_node.unset();
                    return Err(());
                }
            };
        } else {
            // search for new unused backup slot to use
            let mut iter = backup_list.iter();
            let item = iter.find(|_, data| data.is_unused(), storage)?;
            let (slot_location, _) = item.expect("There should be a slot left, as ResidentObjectManager always makes sure that MetadataBackupList.len() >= ResidentObjects.len()");

            self.inner
                .metadata_backup_node
                .set(slot_location.get_data_offset());

            match self.write_metadata(storage, slot_location.get_data_offset(), false) {
                Ok(()) => {}
                Err(()) => {
                    // metadata could not be saved, reset this slot
                    let backup_data = ResidentObjectMetadataBackup::new_unused();
                    write_storage_data(storage, slot_location.get_data_offset(), &backup_data)?;

                    self.inner.metadata_backup_node.unset();
                    return Err(());
                }
            };

            // save the fact that we saved our metadata to that offset
            self.inner
                .metadata_backup_node
                .set(slot_location.get_data_offset());
        }

        self.inner.dirty_status.set_general_metadata_dirty(false);

        Ok(UNSYNCED_METADATA_DIRTY_SIZE - SYNCED_METADATA_DIRTY_SIZE)
    }

    pub(crate) fn write_metadata<S: PersistentStorageModule>(
        &self,
        storage: &mut S,
        backup_slot: usize,
        metadata_dirty: bool,
    ) -> Result<(), ()> {
        let mut backup_data = ResidentObjectMetadataBackup::from_metadata(self);
        backup_data
            .inner
            .dirty_status
            .set_general_metadata_dirty(metadata_dirty);
        write_storage_data(storage, backup_slot, &backup_data)?;

        Ok(())
    }

    pub(crate) fn remove_metadata_backup<S: PersistentStorageModule>(
        &mut self,
        storage: &mut S,
    ) -> Result<usize, ()> {
        if let Some(node) = self.inner.metadata_backup_node.get() {
            let prev_dirty = self.inner.dirty_status.is_general_metadata_dirty();

            self.inner.dirty_status.set_general_metadata_dirty(true);
            unsafe { ResidentObjectMetadataBackup::make_unused(node, storage) }?;

            if prev_dirty {
                // was already dirty previously, no change of dirty size
                Ok(0)
            } else {
                // is dirty now, dirty size is changed
                Ok(UNSYNCED_METADATA_DIRTY_SIZE - SYNCED_METADATA_DIRTY_SIZE)
            }
        } else {
            Ok(0)
        }
    }
}
/*
impl ResidentObjectMetadata {
    pub(super) fn new<T: Sized>(offset: usize) -> Self {
        ResidentObjectMetadata {
            next_dirty_object: null_mut(),
            next_resident_object: null_mut(),
            inner: ResidentObjectMetadataInner::new::<T>(offset)
        }
    }

    fn dirty_size(&self) -> usize {
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

    /// ### Safety
    ///
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    #[inline]
    unsafe fn dynamic_metadata_to_data_range(&self) -> &[u8] {
        let meta_ptr = ((self as *const ResidentObjectMetadata) as *const u8)
            .add(size_of::<ResidentObjectMetadata>());

        // align base pointer (add alignment, because T could be aligned)
        let base_ptr = ((meta_ptr as usize) + (self.inner.layout.align() - 1)) & !(self.inner.layout.align() - 1);

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

        slice_from_raw_parts(base_ptr, self.inner.layout.size())
            .as_ref()
            .unwrap()
    }

    /// Reinterpret metadata as whole resident object.
    ///
    /// ### Safety
    ///
    /// It is only safe to read/write the returned pointer if I you previously create a `ResidentObject` and
    /// are now calling this function on its `metadata` member.
    ///
    /// Also: You should not have any other mutable references to this `ResidentObject`
    #[inline]
    pub(super) unsafe fn metadata_to_resident_obj_ptr<T>(ptr: *mut ResidentObjectMetadata) -> *mut ResidentObject<T> {
        ptr as *mut ResidentObject<T>
    }

    /// The same as `metadata_to_resident_obj_ptr` but without the type `T`
    #[inline]
    pub(super) unsafe fn metadata_to_resident_obj_ptr_base(ptr: *mut ResidentObjectMetadata) -> *mut u8 {
        ptr as *mut u8
    }

    #[inline]
    pub(super) unsafe fn ptr_from_meta_inner_mut(ptr: *mut ResidentObjectMetadataInner) -> *mut ResidentObjectMetadata {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, inner);
        (unsafe { (ptr as *mut u8).sub(OFFSET) }) as *mut ResidentObjectMetadata
    }
    #[inline]
    pub(super) unsafe fn ptr_from_meta_inner(ptr: *const ResidentObjectMetadataInner) -> *const ResidentObjectMetadata {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, inner);
        (unsafe { (ptr as *mut u8).sub(OFFSET) }) as *mut ResidentObjectMetadata
    }

    #[inline]
    pub(super) fn get_next_resident_item(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut MultiLinkedListDefaultPointer<ResidentObjectMetadata> {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, next_resident_object);
        (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut MultiLinkedListDefaultPointer<ResidentObjectMetadata>
    }

    #[inline]
    pub(super) fn get_next_dirty_item(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut MultiLinkedListAtomicPointer<ResidentObjectMetadata> {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, next_dirty_object);
        (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut MultiLinkedListAtomicPointer<ResidentObjectMetadata>
    }

    #[inline]
    pub(super) fn get_inner(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut ResidentObjectMetadataInner {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, inner);
        (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut ResidentObjectMetadataInner
    }
}
*/

#[cfg(test)]
mod test {
    use core::{alloc::Layout, mem::size_of};

    use crate::resident_object_manager::resident_object::{
        calc_resident_obj_layout, ResidentObject, ResidentObjectMetadata,
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
    }

    fn test_calc_resident_obj_layout_internal<T: Sized>() {
        assert_eq!(
            Layout::new::<ResidentObject<T>>(),
            calc_resident_obj_layout(Layout::new::<T>())
        );
    }

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
