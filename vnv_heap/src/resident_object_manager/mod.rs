use core::{alloc::Layout, marker::PhantomData, mem::size_of, ptr::NonNull};

use log::{debug, error, trace, warn};
use resident_list::ResidentList;

use crate::modules::object_management::{
    DirtyItemList, DirtyItemListArguments, ObjectManagementModule, ResidentItemList,
    ResidentItemListArguments,
};
use crate::modules::persistent_storage::persistent_storage_util::read_storage_data;
use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        persistent_storage::PersistentStorageModule,
    },
};

mod dirty_status;
mod metadata_backup_info;
mod metadata_backup_list;
mod persist;
pub(crate) mod resident_list;
pub(crate) mod resident_object;
mod resident_object_backup;

pub(crate) use metadata_backup_list::{MetadataBackupList, SharedMetadataBackupPtr};
pub(crate) use persist::*;
use resident_object::*;
use resident_object_backup::*;

#[cfg(test)]
mod test;

/// Calculate the size that is cut off the resident buffer
pub(crate) const fn calc_resident_manager_total_cutoff() -> usize {
    MetadataBackupList::total_item_size()
}

pub(crate) struct ResidentObjectManager<'a, A: AllocatorModule, M: ObjectManagementModule> {
    /// In memory heap for resident objects and their metadata
    pub(crate) heap: A,

    /// Allocated list inside non volatile storage
    /// to backup metadata when PFI occurs
    ///
    /// Following is always true: `resident_object_meta_backup.len() >= resident_object_count`
    pub(crate) resident_object_meta_backup: MetadataBackupList,

    /// How many objects are currently resident?
    pub(crate) resident_object_count: usize,

    /// How many bytes can still be made dirty without
    /// violating users requirements
    pub(crate) remaining_dirty_size: usize,

    /// List of objects that are currently resident
    resident_list: ResidentList,

    /// Object management module
    object_manager: M,

    /// Phantom data to resident buffer, to bind its lifetime to `ResidentObjectManager`
    _resident_buffer: PhantomData<&'a mut [u8]>,

    /// How many bytes should be able to be dirty initially
    /// Used for debugging
    #[cfg(debug_assertions)]
    pub(crate) _initial_dirty_size: usize,
}

impl<'a, A: AllocatorModule, M: ObjectManagementModule> ResidentObjectManager<'a, A, M> {
    /// Create a new resident object manager
    ///
    /// **Note**: Will overwrite any data, at index 0 of the given persistent storage.
    ///
    /// Returns the newly created instance and the offset from which on data can
    /// be stored to persistent storage safely again.
    pub(crate) fn new<S: PersistentStorageModule>(
        resident_buffer: &'a mut [u8],
        max_dirty_size: usize,
        storage: &mut S,
    ) -> Result<(Self, usize), ()> {
        let mut heap = A::new();
        unsafe {
            let start_ref = &mut resident_buffer[0];
            heap.init(start_ref, resident_buffer.len());
        }

        // backup item has to be the first in the persistent storage, so restoring is easier
        let mut meta_backup_list = MetadataBackupList::new();

        unsafe { meta_backup_list.push(0, ResidentObjectMetadataBackup::new_unused(), storage) }?;
        let offset = calc_resident_manager_total_cutoff();

        let instance = ResidentObjectManager {
            resident_list: ResidentList::new(),
            heap,
            resident_object_meta_backup: meta_backup_list,
            resident_object_count: 0,
            remaining_dirty_size: max_dirty_size,
            object_manager: M::new(),
            _resident_buffer: PhantomData,

            #[cfg(debug_assertions)]
            _initial_dirty_size: max_dirty_size,
        };

        Ok((instance, offset))
    }
}

impl<A: AllocatorModule, M: ObjectManagementModule> ResidentObjectManager<'_, A, M> {
    /// Makes the given object resident if not already and returns a pointer to the resident data
    unsafe fn require_resident<
        T: Sized,
        S: PersistentStorageModule,
        N: NonResidentAllocatorModule,
    >(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        non_resident_allocator: &mut N,
        storage: &mut S,
    ) -> Result<&mut ResidentObject<T>, ()> {
        if let Some(metadata) = self.find_element_mut(&alloc_id) {
            // already resident
            let res_object_ptr = ResidentObjectMetadata::ptr_to_resident_obj_ptr(metadata);
            return Ok(res_object_ptr.as_mut().unwrap());
        }

        trace!("Make object resident (offset: {})", alloc_id.offset);

        debug_assert!(
            self.resident_object_count <= self.resident_object_meta_backup.len(),
            "requirement should not be violated (resident_object_count={}, resident_object_meta_backup.len()={})", self.resident_object_count, self.resident_object_meta_backup.len()
        );
        if self.resident_object_count == self.resident_object_meta_backup.len() {
            // acquire new slot for backup
            let ptr =
                non_resident_allocator.allocate(MetadataBackupList::item_layout(), storage)?;

            self.resident_object_meta_backup.push(
                ptr,
                ResidentObjectMetadataBackup::new_unused(),
                storage,
            )?;
        }

        let obj_ptr = self.allocate_resident_space(Layout::new::<ResidentObject<T>>(), storage)?;

        // metadata will be regarded dirty the moment the object is made persistently
        if self.remaining_dirty_size < ResidentObjectMetadata::fresh_object_dirty_size() {
            // not enough dirty bytes remaining
            // sync some data now by using object manager
            let required_bytes =
                ResidentObjectMetadata::fresh_object_dirty_size() - self.remaining_dirty_size;
            self.sync_dirty_data(required_bytes, storage)?;
        }
        self.remaining_dirty_size -= ResidentObjectMetadata::fresh_object_dirty_size();

        // read data now and store it to the allocated region in memory
        let obj_ptr = obj_ptr.as_ptr() as *mut ResidentObject<T>;

        let data: T = read_storage_data(storage, alloc_id.offset)?;
        let obj = ResidentObject {
            data,
            metadata: ResidentObjectMetadata::new::<T>(alloc_id.offset),
        };
        debug_assert_eq!(
            ResidentObjectMetadata::fresh_object_dirty_size(),
            obj.metadata.dirty_size(),
            "Dirty size of newly created metadata should match const value"
        );

        obj_ptr.write(obj);

        let obj_ref = obj_ptr.as_mut().unwrap();

        self.resident_list.push(&mut obj_ref.metadata);
        self.resident_object_count += 1;
        Ok(obj_ref)
    }

    /// Makes the object non resident.
    ///
    /// If `T` requires dropping, the object is made resident first and then dropped afterwards.
    pub(crate) fn drop<T: Sized, N: NonResidentAllocatorModule, S: PersistentStorageModule>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        non_resident_allocator: &mut N,
        storage: &mut S,
    ) -> Result<(), ()> {
        self.check_integrity();
        if core::mem::needs_drop::<T>() {
            // require resident to drop object in memory
            let _ = unsafe { self.require_resident(alloc_id, non_resident_allocator, storage) }?;
        }

        let mut iter_mut = self.resident_list.iter_mut();
        while let Some(mut curr) = iter_mut.next() {
            let found = {
                // important: drop the item reference here
                // so we can iterate over the dirty list later
                // (without having two mutable references to the same data)
                let item_ref = curr.get_element();

                item_ref.inner.offset == alloc_id.offset
            };

            if found {
                unsafe {
                    ResidentObject::<T>::unload_resident_object(
                        curr,
                        storage,
                        &mut self.heap,
                        &mut self.remaining_dirty_size,
                    )?
                }

                self.resident_object_count -= 1;

                self.check_integrity();
                return Ok(());
            }
        }

        // if this point is reached
        // it means that this object was not resident

        if core::mem::needs_drop::<T>() {
            // should not happen: object should be made resident and dropped in RAM
            debug_assert!(false, "Should not happen");
            Err(())
        } else {
            self.check_integrity();

            // everything is fine, object was not resident
            // but does not need to be dropped (because T does not require so)
            Ok(())
        }
    }

    pub(crate) unsafe fn get_mut<
        T: Sized,
        S: PersistentStorageModule,
        N: NonResidentAllocatorModule,
    >(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        non_resident_allocator: &mut N,
        storage: &mut S,
    ) -> Result<*mut T, ()> {
        self.check_integrity();
        trace!("Get mutable reference (offset={})", identifier.offset);

        let obj_ref: *mut ResidentObject<T> =
            self.require_resident(identifier, non_resident_allocator, storage)?;

        let bytes_to_sync = {
            let meta_ref = &mut obj_ref.as_mut().unwrap().metadata;

            // should be ensured by the rust compiler
            debug_assert_eq!(
                meta_ref.inner.ref_cnt, 0,
                "There should be no references to this object yet!"
            );

            if !meta_ref.inner.dirty_status.is_data_dirty()
                && self.remaining_dirty_size < meta_ref.inner.layout.size()
            {
                // was previously not dirty and
                // not enough space left to make it dirty
                meta_ref.inner.layout.size() - self.remaining_dirty_size
            } else {
                0
            }
        };

        // its IMPORTANT here that we don't have any open reference to a ResidentObject/ResidentObjectMetadata anymore
        if bytes_to_sync != 0 {
            // sync data now
            self.sync_dirty_data(bytes_to_sync, storage)?;
        }

        let obj_ref = obj_ref.as_mut().unwrap();
        let meta_ref = &mut obj_ref.metadata;

        if !meta_ref.inner.dirty_status.is_data_dirty() {
            // make dirty
            self.remaining_dirty_size -= meta_ref.inner.layout.size();
            meta_ref.inner.dirty_status.set_data_dirty(true);
        }

        meta_ref.inner.ref_cnt = usize::MAX;

        Ok(&mut obj_ref.data)
    }

    pub(crate) unsafe fn get_ref<
        T: Sized,
        S: PersistentStorageModule,
        N: NonResidentAllocatorModule,
    >(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        non_resident_allocator: &mut N,
        storage: &mut S,
    ) -> Result<*const T, ()> {
        self.check_integrity();
        trace!("Get mutable reference (offset={})", identifier.offset);

        let obj_ref = self.require_resident(identifier, non_resident_allocator, storage)?;
        let meta_ref = &mut obj_ref.metadata.inner;

        debug_assert_ne!(
            meta_ref.ref_cnt,
            usize::MAX,
            "There should be no mutable references to this object!"
        );

        if meta_ref.ref_cnt >= usize::MAX - 1 {
            // too many references?
            error!(
                "Cannot request ref for resident object: Too many references (current ref_cnt: {})",
                meta_ref.ref_cnt
            );
            self.check_integrity();
            return Err(());
        }

        meta_ref.ref_cnt += 1;

        Ok(&mut obj_ref.data)
    }

    pub(crate) unsafe fn release_mut<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        _data: &mut T,
    ) {
        self.check_integrity();
        trace!("Release mutable reference (offset={})", identifier.offset);
        if let Some(meta_ptr) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
            let meta_ref = &mut meta_ref.inner;
            debug_assert_eq!(meta_ref.ref_cnt, usize::MAX);

            meta_ref.ref_cnt = 0;
        } else {
            // nothing to do, as references are not tracked for nonresident objects
            // should not happen anyway...
            debug_assert!(
                false,
                "Released mutable reference to a non resident object (offset: {})!",
                identifier.offset
            );
        }
        self.check_integrity();
    }

    pub(crate) unsafe fn release_ref<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        _data: &T,
    ) {
        self.check_integrity();
        trace!("Release immutable reference (offset={})", identifier.offset);
        if let Some(meta_ptr) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
            let meta_ref = &mut meta_ref.inner;
            debug_assert_ne!(meta_ref.ref_cnt, 0);
            debug_assert_ne!(meta_ref.ref_cnt, usize::MAX);

            meta_ref.ref_cnt -= 1;
        } else {
            // nothing to do, as references are not tracked for nonresident objects
            // should not happen anyway...
            debug_assert!(
                false,
                "Released reference to a non resident object (offset: {})!",
                identifier.offset
            );
        }
        self.check_integrity();
    }

    /// Flushes as many bytes until at least: `remaining_dirty_bytes >= required_bytes`.
    ///
    /// ### Safety
    ///
    /// As this list modifies the `dirty_list`, make sure there are no open (mutable) references
    #[inline]
    unsafe fn sync_dirty_data<S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        storage: &mut S,
    ) -> Result<(), ()> {
        if required_bytes == 0 {
            return Ok(());
        }

        let prev_dirty_size = self.remaining_dirty_size;

        let mut args = DirtyItemListArguments {
            remaining_dirty_size: &mut self.remaining_dirty_size,
            resident_object_meta_backup: &mut self.resident_object_meta_backup,
            storage: storage,
        };

        let dirty_list = DirtyItemList {
            arguments: &mut args,
            resident_list: &mut self.resident_list,
        };

        self.object_manager
            .sync_dirty_data(required_bytes, dirty_list)?;
        assert!(
            self.remaining_dirty_size >= prev_dirty_size + required_bytes,
            "should have made enough space"
        );
        Ok(())
    }

    /// Allocates new space for a resident object.
    /// If no space is available, tries to make other unused objects non resident
    #[inline]
    unsafe fn allocate_resident_space<S: PersistentStorageModule>(
        &mut self,
        layout: Layout,
        storage: &mut S,
    ) -> Result<NonNull<u8>, ()> {
        // try to allocate
        match self.heap.allocate(layout) {
            Ok(res) => return Ok(res),
            Err(_) => {}
        }

        debug!(
            "Could not allocate {} bytes in RAM, try to make some other objects non resident...",
            layout.size()
        );

        let mut args = ResidentItemListArguments {
            allocator: &mut self.heap,
            remaining_dirty_size: &mut self.remaining_dirty_size,
            resident_object_count: &mut self.resident_object_count,
            storage,
        };

        let list = ResidentItemList {
            arguments: &mut args,
            resident_list: &mut self.resident_list,
        };

        if let Ok(()) = self.object_manager.unload_objects(&layout, list) {
            debug!(
                "-> Success! Made Enough objects resident to allocate {} bytes in RAM",
                layout.size()
            );

            Ok(self
                .heap
                .allocate(layout)
                .expect("unload_objects should made sure that there is enough space"))
        } else {
            warn!(
                "-> Could not allocate an object with size {} in RAM",
                layout.size()
            );

            Err(())
        }
    }

    /// Finds an object in the resident list
    ///
    /// ### Safety
    ///
    /// This function is only safe to call if there aren't any open
    /// references to ResidentObjectMetadataInner objects!
    #[inline]
    unsafe fn find_element_mut<T: Sized>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
    ) -> Option<*mut ResidentObjectMetadata> {
        let mut iter = self.resident_list.iter_mut();
        while let Some(mut item) = iter.next() {
            let item_ref = item.get_element();
            if item_ref.inner.offset == alloc_id.offset {
                return Some(item_ref);
            }
        }
        None
    }

    #[cfg(debug_assertions)]
    fn check_integrity(&self) {
        // check if resident_object_count and remaining_dirty_size are correct
        let mut obj_cnt = 0;
        let mut dirty_size = 0;

        let mut iter = self.resident_list.iter();
        while let Some(item) = iter.next() {
            obj_cnt += 1;
            dirty_size += item.dirty_size();
        }

        assert_eq!(obj_cnt, self.resident_object_count);
        assert_eq!(
            dirty_size + self.remaining_dirty_size,
            self._initial_dirty_size
        );
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    fn check_integrity(&self) {
        // check nothing
    }
}

impl<A: AllocatorModule, M: ObjectManagementModule> ResidentObjectManager<'_, A, M> {
    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_remaining_dirty_size(&self) -> usize {
        self.remaining_dirty_size
    }
}

pub(crate) const fn get_total_resident_size<T: Sized>() -> usize {
    size_of::<ResidentObject<T>>()
}
