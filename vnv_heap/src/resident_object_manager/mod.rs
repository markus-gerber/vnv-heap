use core::ptr::slice_from_raw_parts_mut;
use core::{marker::PhantomData, mem::size_of};

use log::{debug, trace, warn};
use memoffset::offset_of;
use resident_list::ResidentList;
use resident_object_metadata::ResidentObjectMetadata;

use crate::modules::object_management::{
    DirtyItemList, DirtyItemListArguments, ObjectManagementModule, ResidentItemList,
    ResidentItemListArguments,
};
use crate::shared_persist_lock::SharedPersistLock;
use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule},
};

pub(crate) mod partial_dirtiness_tracking;
mod persist;
pub(crate) mod resident_list;
pub(crate) mod resident_object;
pub(crate) mod resident_object_backup;
pub(crate) mod resident_object_metadata;
mod resident_object_status;

pub(crate) use persist::*;
use resident_object::*;
use resident_object_backup::*;

#[cfg(test)]
mod test;

pub(crate) struct ResidentObjectManager<'a: 'b, 'b, A: AllocatorModule, M: ObjectManagementModule> {
    /// In memory heap for resident objects and their metadata
    pub(crate) heap: SharedPersistLock<'b, *mut A>,

    /// How many bytes can still be made dirty without
    /// violating users requirements
    pub(crate) remaining_dirty_size: usize,

    /// List of objects that are currently resident
    resident_list: &'b mut ResidentList,

    /// Object management module
    object_manager: M,

    /// Phantom data to resident buffer, to bind its lifetime to `ResidentObjectManager`
    _resident_buffer: PhantomData<&'a mut [u8]>,

    /// How many bytes should be able to be dirty initially
    /// Used for debugging
    #[cfg(debug_assertions)]
    pub(crate) _initial_dirty_size: usize,
}

impl<'a, 'b, A: AllocatorModule, M: ObjectManagementModule> ResidentObjectManager<'a, 'b, A, M> {
    /// Create a new resident object manager
    ///
    /// **Note**: Will overwrite any data, at index 0 of the given persistent storage.
    ///
    /// Returns the newly created instance and the offset from which on data can
    /// be stored to persistent storage safely again.
    pub(crate) fn new(
        resident_buffer: &'a mut [u8],
        max_dirty_size: usize,
        resident_list: &'b mut ResidentList,
        heap: SharedPersistLock<'b, *mut A>,
    ) -> Result<Self, ()> {
        {
            // init heap
            let guard = heap.try_lock().unwrap();

            let start_ref = &mut resident_buffer[0];
            unsafe {
                guard
                    .as_mut()
                    .unwrap()
                    .init(start_ref, resident_buffer.len())
            };

            /*
            // dirty way to test which object sizes can be allocated and which cannot
            unsafe {
                let x = guard.as_mut().unwrap();
                seq_macro::seq!(I in 896..1024 {
                    {
                        match x.allocate(std::alloc::Layout::new::<ResidentObject<[u8; I]>>()) {
                            Err(()) => {
                                println!("{}: error, size: {}, align: {}", I, std::alloc::Layout::new::<ResidentObject<[u8; I]>>().size(), std::alloc::Layout::new::<ResidentObject<[u8; I]>>().align());
                            },
                            Ok(ptr) => {
                                println!("{}: success, size: {}, align: {}", I, std::alloc::Layout::new::<ResidentObject<[u8; I]>>().size(), std::alloc::Layout::new::<ResidentObject<[u8; I]>>().align());
                                x.deallocate(ptr, std::alloc::Layout::new::<ResidentObject<[u8; I]>>());
                            }
                        }


                    }
                });
            }
            */
        }

        let instance = ResidentObjectManager {
            resident_list,
            heap,
            remaining_dirty_size: max_dirty_size,
            object_manager: M::new(),
            _resident_buffer: PhantomData,

            #[cfg(debug_assertions)]
            _initial_dirty_size: max_dirty_size,
        };

        Ok(instance)
    }
}

impl<A: AllocatorModule, M: ObjectManagementModule> ResidentObjectManager<'_, '_, A, M> {
    /// Makes the given object resident if not already and returns a pointer to the resident data
    unsafe fn require_resident<T: Sized, S: PersistentStorageModule>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        enable_partial_dirtiness_tracking: bool,
        storage: &mut S,
    ) -> Result<&mut ResidentObject<T>, ()> {
        if let Some(metadata) = self.find_element_mut(&alloc_id) {
            // already resident
            let res_object_ptr = ResidentObjectMetadata::ptr_to_resident_obj_ptr(metadata);
            return Ok(res_object_ptr.as_mut().unwrap());
        }

        trace!("Make object resident (offset: {})", alloc_id.offset);

        let (total_layout, res_obj_offset) =
            calc_resident_obj_layout_static::<T>(enable_partial_dirtiness_tracking);

        let (mut obj_ptr, mut guard) = {
            // try to allocate

            // unwrap is okay here because there are no other threads concurrently accessing it
            // except from vnv_persist_all, but as it is guaranteed that no other threads run
            // during its execution, it is fine
            let guard = self.heap.try_lock().unwrap();

            match guard.as_mut().unwrap().allocate(total_layout) {
                Ok(res) => (res, guard),
                Err(_) => {
                    drop(guard);

                    debug!(
                        "Could not allocate {} bytes in RAM, try to make some other objects non resident...",
                        total_layout.size()
                    );

                    let mut args = ResidentItemListArguments {
                        allocator: &self.heap,
                        remaining_dirty_size: &mut self.remaining_dirty_size,
                        storage,
                    };

                    let list = ResidentItemList {
                        arguments: &mut args,
                        resident_list: &mut self.resident_list,
                    };

                    println!("require resident: onload others");
                    if let Ok(()) = self.object_manager.unload_objects(&total_layout, list) {
                        debug!(
                            "-> Success! Made Enough objects resident to allocate {} bytes in RAM",
                            total_layout.size()
                        );

                        {
                            // unwrap is okay here because there are no other threads concurrently accessing it
                            // except from vnv_persist_all, but as it is guaranteed that no other threads run
                            // during its execution, it is fine
                            let guard = self.heap.try_lock().unwrap();
                            (
                                guard.as_mut().unwrap().allocate(total_layout).expect(
                                    "unload_objects should made sure that there is enough space",
                                ),
                                guard,
                            )
                        }
                    } else {
                        warn!(
                            "-> Could not allocate an object with size {} in RAM",
                            total_layout.size()
                        );

                        return Err(());
                    }
                }
            }
        };

        let dirty_size =
            ResidentObjectMetadata::fresh_object_dirty_size::<T>(enable_partial_dirtiness_tracking);

        // metadata will be regarded dirty the moment the object is made persistently
        if self.remaining_dirty_size < dirty_size {
            // deallocate here, because we need to drop guard here and we cant do this while
            // this object is allocated because of race conditions with vnv_persist_all
            guard.as_mut().unwrap().deallocate(obj_ptr, total_layout);

            // drop the guard, as it is used by sync_dirty_data (when unloading objects)
            drop(guard);

            // not enough dirty bytes remaining
            // sync some data now by using object manager
            let required_bytes = dirty_size - self.remaining_dirty_size;

            println!("require resident: sync others");
            sync_dirty_data::<A, S, M>(
                &mut self.remaining_dirty_size,
                self.resident_list,
                &mut self.object_manager,
                required_bytes,
                storage,
                &self.heap,
            )?;

            // reallocate
            // this should not fail as we previously already allocated the space
            guard = self.heap.try_lock().unwrap();
            obj_ptr = guard.as_mut().unwrap().allocate(total_layout).unwrap();
        }

        self.remaining_dirty_size -= dirty_size;

        // read data now and store it to the allocated region in memory
        let resident_obj_ptr = obj_ptr.as_ptr().add(res_obj_offset);

        let meta_ptr = resident_obj_ptr.add(offset_of!(ResidentObject<T>, metadata))
            as *mut ResidentObjectMetadata;
        meta_ptr.write(ResidentObjectMetadata::new::<T>(
            alloc_id.offset,
            enable_partial_dirtiness_tracking,
        ));

        {
            // some checks and append to resident list
            let meta_ref = meta_ptr.as_mut().unwrap();

            // this will not do anything if partial dirtiness tracking is disabled
            meta_ref
                .inner
                .partial_dirtiness_tracking_info
                .get_wrapper(meta_ptr)
                .reset();

            debug_assert_eq!(
                dirty_size,
                meta_ref.dirty_size(),
                "Dirty size of newly created metadata should match const value"
            );

            self.resident_list.push(meta_ref);
        }

        // FINISHED WITH CRITICAL ALLOCATE SECTION!
        drop(guard);

        {
            // read object data T
            let obj_data_ptr = resident_obj_ptr.add(offset_of!(ResidentObject<T>, data));
            let data_slice = slice_from_raw_parts_mut(obj_data_ptr, size_of::<T>())
                .as_mut()
                .unwrap();

            match storage.read(
                alloc_id.offset + calc_backup_obj_user_data_offset(),
                data_slice,
            ) {
                Ok(()) => {
                    // success
                }
                Err(()) => {
                    // error: deallocate again

                    // unwrap is okay here because there are no other threads concurrently accessing it
                    // except from vnv_persist_all, but as it is guaranteed that no other threads run
                    // during its execution, it is fine
                    let guard = self.heap.try_lock().unwrap();

                    // pop previously created resident object
                    let _ = self.resident_list.pop();

                    guard.as_mut().unwrap().deallocate(obj_ptr, total_layout);
                    drop(guard);

                    self.remaining_dirty_size += dirty_size;

                    self.check_integrity();
                    return Err(());
                }
            };
        }

        let obj_ref = (resident_obj_ptr as *mut ResidentObject<T>)
            .as_mut()
            .unwrap();

        Ok(obj_ref)
    }

    pub(crate) fn unload_object<T: Sized, S: PersistentStorageModule>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        storage: &mut S,
        user_partial_dirtiness_tracking: bool,
    ) -> Result<(), ()> {
        self.check_integrity();

        let mut iter = self.resident_list.iter_mut();
        while let Some(mut element) = iter.next() {
            if element.get_element().inner.offset == alloc_id.offset {
                // element found
                {
                    let element_ref = element.get_element();
                    if element_ref.inner.status.is_in_use() {
                        return Err(());
                    }
                }

                unsafe {
                    ResidentObject::<T>::unload_resident_object(
                        element,
                        storage,
                        &mut self.heap,
                        &mut self.remaining_dirty_size,
                        false,
                        user_partial_dirtiness_tracking,
                    )
                    .expect("unloading should succeed")
                };

                break;
            }
        }

        self.check_integrity();

        return Ok(());
    }

    pub(crate) fn try_to_allocate<T>(
        &mut self,
        data: T,
        storage_base_offset: usize,
        storage_metadata_offset: usize,
        use_partial_dirtiness_tracking: bool,
    ) -> Result<(), T> {
        debug_assert_eq!(
            use_partial_dirtiness_tracking,
            (storage_metadata_offset - storage_base_offset) != 0
        );

        let (resident_obj_layout, resident_metadata_rel_offset) =
            calc_resident_obj_layout_static::<T>(use_partial_dirtiness_tracking);

        let dirty_size = size_of::<T>()
            + ResidentObjectMetadata::fresh_object_dirty_size::<T>(use_partial_dirtiness_tracking);

        if self.remaining_dirty_size < dirty_size {
            // should avoid syncing
            return Err(data);
        }

        let guard = self.heap.try_lock().unwrap();
        let res_ptr = unsafe { guard.as_mut().unwrap().allocate(resident_obj_layout) };
        let res_ptr = match res_ptr {
            Ok(res) => res,
            Err(_) => {
                return Err(data);
            }
        };
        let res_ptr = unsafe { res_ptr.as_ptr().add(resident_metadata_rel_offset) };

        self.remaining_dirty_size -= dirty_size;

        // read data now and store it to the allocated region in memory
        let ptr = res_ptr as *mut ResidentObject<T>;

        let mut metadata = ResidentObjectMetadata::new::<T>(
            storage_metadata_offset,
            use_partial_dirtiness_tracking,
        );
        metadata.inner.status.set_data_dirty(true);
        unsafe { ptr.write(ResidentObject { metadata, data }) };

        {
            // some checks and append to resident list
            let obj_ref = unsafe { ptr.as_mut().unwrap() };

            // this does not do anything if partial dirtiness tracking is disabled
            unsafe {
                obj_ref
                    .metadata
                    .inner
                    .partial_dirtiness_tracking_info
                    .get_wrapper(ptr as *mut ResidentObjectMetadata)
                    .reset_and_set_all_blocks_dirty()
            };

            debug_assert_eq!(
                dirty_size,
                obj_ref.metadata.dirty_size(),
                "Previously calculated dirty size should match the current/actual one"
            );

            unsafe { self.resident_list.push(&mut obj_ref.metadata) };
        }

        drop(guard);

        Ok(())
    }

    /// Makes the object non resident.
    ///
    /// If `T` requires dropping, the object is made resident first and then dropped afterwards.
    pub(crate) fn drop<T: Sized, S: PersistentStorageModule>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
        storage: &mut S,
    ) -> Result<(), ()> {
        self.check_integrity();
        if core::mem::needs_drop::<T>() {
            // require resident to drop object in memory
            let _ = unsafe {
                self.require_resident(alloc_id, use_partial_dirtiness_tracking, storage)
            }?;
        }

        let mut iter_mut = self.resident_list.iter_mut();
        while let Some(mut curr) = iter_mut.next() {
            let found = {
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
                        true,
                        use_partial_dirtiness_tracking,
                    )?
                }

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

    pub(crate) fn is_resident<T>(&mut self, identifier: &AllocationIdentifier<T>) -> bool {
        unsafe { self.find_element_mut(identifier).is_some() }
    }

    pub(crate) fn is_data_dirty<T>(&mut self, identifier: &AllocationIdentifier<T>) -> bool {
        let element = unsafe { self.find_element_mut(identifier) };
        if let Some(element) = element {
            unsafe { element.as_mut().unwrap().inner.status.is_data_dirty() }
        } else {
            false
        }
    }

    pub(crate) unsafe fn get_mut<T: Sized, S: PersistentStorageModule>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
        storage: &mut S,
    ) -> Result<*mut T, ()> {
        self.check_integrity();
        trace!("Get mutable reference (offset={})", identifier.offset);

        let obj_ref: *mut ResidentObject<T> =
            self.require_resident(identifier, use_partial_dirtiness_tracking, storage)?;

        let bytes_to_sync = {
            let meta_ref = &mut obj_ref.as_mut().unwrap().metadata;

            // should be ensured by the rust compiler
            debug_assert!(
                !meta_ref.inner.status.is_in_use(),
                "This object should not be in use yet!"
            );
            debug_assert!(
                !meta_ref.inner.status.is_mutable_ref_active(),
                "This object should not have any mutable references!"
            );

            if !meta_ref.inner.status.is_data_dirty()
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
            sync_dirty_data::<A, S, M>(
                &mut self.remaining_dirty_size,
                self.resident_list,
                &mut self.object_manager,
                bytes_to_sync,
                storage,
                &self.heap,
            )?;
        }

        let obj_ref = obj_ref.as_mut().unwrap();
        let meta_ref = &mut obj_ref.metadata;

        if !meta_ref.inner.status.is_data_dirty() {
            assert!(self.remaining_dirty_size >= meta_ref.inner.layout.size());

            // make dirty
            self.remaining_dirty_size -= meta_ref.inner.layout.size();
            meta_ref.inner.status.set_data_dirty(true);
        }

        meta_ref.inner.status.set_is_in_use(true);
        meta_ref.inner.status.set_is_mutable_ref_active(true);
        self.check_integrity();

        Ok(&mut obj_ref.data)
    }

    pub(crate) unsafe fn get_partial_mut<T: Sized, S: PersistentStorageModule>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        storage: &mut S,
    ) -> Result<(*mut ResidentObjectMetadata, *mut T), ()> {
        self.check_integrity();
        trace!(
            "Get partial mutable reference (offset={})",
            identifier.offset
        );

        let obj_ref: *mut ResidentObject<T> = self.require_resident(identifier, true, storage)?;

        let obj_ref = obj_ref.as_mut().unwrap();
        let meta_ref = &mut obj_ref.metadata;

        // Should be enforced by the rust compiler (as long the VNVList is implemented correctly)
        debug_assert!(!meta_ref.inner.status.is_in_use(), "Should not be in use!");
        debug_assert!(
            !meta_ref.inner.status.is_mutable_ref_active(),
            "Should have no open mutable references!"
        );

        meta_ref.inner.status.set_is_in_use(true);
        meta_ref.inner.status.set_is_mutable_ref_active(true);
        self.check_integrity();

        Ok((&mut obj_ref.metadata, &mut obj_ref.data))
    }

    pub(crate) fn partial_mut_make_range_dirty<S: PersistentStorageModule>(
        &mut self,
        meta_ref: &mut ResidentObjectMetadata,
        addr_offset: usize,
        size: usize,
        storage: &mut S,
    ) -> Result<(), ()> {
        self.check_integrity();

        // some important checks
        debug_assert!(meta_ref
            .inner
            .status
            .is_partial_dirtiness_tracking_enabled());
        debug_assert!(meta_ref.inner.status.is_in_use());
        debug_assert!(meta_ref.inner.status.is_mutable_ref_active());

        let mut wrapper = unsafe {
            meta_ref
                .inner
                .partial_dirtiness_tracking_info
                .get_wrapper(meta_ref)
        };
        let dirty_size = wrapper.get_non_dirty_size_in_range(addr_offset, size);

        if dirty_size > self.remaining_dirty_size {
            // remaining dirty size is too small to make this data region dirty
            // try to sync enough dirty data now
            let bytes_to_sync = dirty_size - self.remaining_dirty_size;

            unsafe {
                sync_dirty_data::<A, S, M>(
                    &mut self.remaining_dirty_size,
                    self.resident_list,
                    &mut self.object_manager,
                    bytes_to_sync,
                    storage,
                    &self.heap,
                )?;
            }
        }

        debug_assert!(
            self.remaining_dirty_size >= dirty_size,
            "{} >= {}",
            self.remaining_dirty_size,
            dirty_size
        );
        self.remaining_dirty_size -= dirty_size;

        // yay, we can make all bytes in the range dirty
        wrapper.set_range_dirty(addr_offset, size);
        meta_ref.inner.status.set_data_dirty(true);

        self.check_integrity();

        Ok(())
    }

    pub(crate) unsafe fn release_partial_mut<T: Sized>(
        &mut self,
        meta_ptr: *mut ResidentObjectMetadata,
    ) {
        self.check_integrity();
        let meta_ref = meta_ptr.as_mut().unwrap();
        let meta_ref = &mut meta_ref.inner;
        trace!(
            "Release partial mutable reference (offset={})",
            meta_ref.offset
        );
        debug_assert!(meta_ref.status.is_in_use());
        debug_assert!(meta_ref.status.is_mutable_ref_active());

        meta_ref.status.set_is_in_use(false);
        meta_ref.status.set_is_mutable_ref_active(false);
        self.check_integrity();
    }

    pub(crate) unsafe fn get_ref<T: Sized, S: PersistentStorageModule>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
        storage: &mut S,
    ) -> Result<*const T, ()> {
        self.check_integrity();
        trace!("Get mutable reference (offset={})", identifier.offset);

        let obj_ref = self.require_resident(identifier, use_partial_dirtiness_tracking, storage)?;
        let meta_ref = &mut obj_ref.metadata.inner;

        debug_assert!(
            !meta_ref.status.is_in_use(),
            "This object should not be in use yet!"
        );
        debug_assert!(
            !meta_ref.status.is_mutable_ref_active(),
            "This object should not have any mutable references!"
        );

        meta_ref.status.set_is_in_use(true);

        Ok(&mut obj_ref.data)
    }

    pub(crate) unsafe fn release_mut<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) {
        self.check_integrity();
        trace!("Release mutable reference (offset={})", identifier.offset);
        if let Some(meta_ptr) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
            let meta_ref = &mut meta_ref.inner;
            debug_assert!(meta_ref.status.is_in_use());
            debug_assert!(meta_ref.status.is_mutable_ref_active());

            meta_ref.status.set_is_in_use(false);
            meta_ref.status.set_is_mutable_ref_active(false);
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

    pub(crate) unsafe fn release_ref<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) {
        self.check_integrity();
        trace!("Release immutable reference (offset={})", identifier.offset);
        if let Some(meta_ptr) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
            let meta_ref = &mut meta_ref.inner;
            debug_assert!(meta_ref.status.is_in_use());
            debug_assert!(!meta_ref.status.is_mutable_ref_active());

            meta_ref.status.set_is_in_use(false);
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

    pub(crate) fn count_resident_objects(&self) -> usize {
        self.resident_list.iter().count() 
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
        let mut dirty_size = 0;

        let mut iter = self.resident_list.iter();
        while let Some(item) = iter.next() {
            dirty_size += item.dirty_size();
        }

        assert_eq!(
            dirty_size + self.remaining_dirty_size,
            self._initial_dirty_size,
            "did not match initial dirty size: curr_dirty_size: {}, remaining_dirty_size: {}, initial_dirty_size: {}",
            dirty_size,
            self.remaining_dirty_size,
            self._initial_dirty_size
        );
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    fn check_integrity(&self) {
        // check nothing
    }
}

impl<A: AllocatorModule, M: ObjectManagementModule> ResidentObjectManager<'_, '_, A, M> {
    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_remaining_dirty_size(&self) -> usize {
        self.remaining_dirty_size
    }
}

#[allow(dead_code)]
pub(crate) const fn get_total_resident_size<T: Sized>() -> usize {
    size_of::<ResidentObject<T>>()
}

/// Flushes as many bytes until at least: `remaining_dirty_bytes >= required_bytes`.
///
/// ### Safety
///
/// As this list modifies the `resident_list`, make sure there are no open (mutable) references
#[inline]
unsafe fn sync_dirty_data<
    'a,
    'b,
    A: AllocatorModule,
    S: PersistentStorageModule,
    M: ObjectManagementModule,
>(
    remaining_dirty_size: &'a mut usize,
    resident_list: &'a mut ResidentList,
    object_manager: &'a mut M,
    required_bytes: usize,
    storage: &'a mut S,
    allocator: &'a SharedPersistLock<'b, *mut A>,
) -> Result<(), ()> {
    if required_bytes == 0 {
        return Ok(());
    }

    let prev_dirty_size = *remaining_dirty_size;

    let mut args = DirtyItemListArguments {
        remaining_dirty_size,
        storage: storage,
        allocator,
    };

    let dirty_list = DirtyItemList {
        arguments: &mut args,
        resident_list,
    };

    object_manager.sync_dirty_data(required_bytes, dirty_list)?;
    assert!(
        *remaining_dirty_size >= prev_dirty_size + required_bytes,
        "should have made enough space"
    );
    Ok(())
}
