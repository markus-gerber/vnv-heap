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
    modules::{
        allocator::AllocatorModule, persistent_storage::PersistentStorageModule,
    },
};

mod resident_object_status;
mod persist;
pub(crate) mod resident_list;
pub(crate) mod resident_object;
pub(crate) mod resident_object_backup;
pub(crate) mod resident_object_metadata;

pub(crate) use persist::*;
use resident_object::*;
use resident_object_backup::*;

#[cfg(test)]
mod test;

pub(crate) struct ResidentObjectManager<
    'a: 'b,
    'b,
    A: AllocatorModule,
    M: ObjectManagementModule,
> {
    /// In memory heap for resident objects and their metadata
    pub(crate) heap: SharedPersistLock<'b, *mut A>,

    /// How many objects are currently resident?
    pub(crate) resident_object_count: usize,

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

impl<'a, 'b, A: AllocatorModule, M: ObjectManagementModule>
    ResidentObjectManager<'a, 'b, A, M>
{
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
            resident_object_count: 0,
            remaining_dirty_size: max_dirty_size,
            object_manager: M::new(),
            _resident_buffer: PhantomData,

            #[cfg(debug_assertions)]
            _initial_dirty_size: max_dirty_size,
        };

        Ok(instance)
    }
}

impl<A: AllocatorModule, M: ObjectManagementModule>
    ResidentObjectManager<'_, '_, A, M>
{
    /// Makes the given object resident if not already and returns a pointer to the resident data
    unsafe fn require_resident<
        T: Sized,
        S: PersistentStorageModule,
    >(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        storage: &mut S,
    ) -> Result<&mut ResidentObject<T>, ()> {
        if let Some(metadata) = self.find_element_mut(&alloc_id) {
            // already resident
            let res_object_ptr = ResidentObjectMetadata::ptr_to_resident_obj_ptr(metadata);
            return Ok(res_object_ptr.as_mut().unwrap());
        }

        trace!("Make object resident (offset: {})", alloc_id.offset);

        let obj_layout = calc_resident_obj_layout_static::<T>();

        let (mut obj_ptr, mut guard) = {
            // try to allocate

            // unwrap is okay here because there are no other threads concurrently accessing it
            // except from vnv_persist_all, but as it is guaranteed that no other threads run
            // during its execution, it is fine
            let guard = self.heap.try_lock().unwrap();

            match guard.as_mut().unwrap().allocate(obj_layout) {
                Ok(res) => (res, guard),
                Err(_) => {
                    drop(guard);

                    debug!(
                        "Could not allocate {} bytes in RAM, try to make some other objects non resident...",
                        obj_layout.size()
                    );

                    let mut args = ResidentItemListArguments {
                        allocator: &self.heap,
                        remaining_dirty_size: &mut self.remaining_dirty_size,
                        resident_object_count: &mut self.resident_object_count,
                        storage,
                    };

                    let list = ResidentItemList {
                        arguments: &mut args,
                        resident_list: &mut self.resident_list,
                    };

                    if let Ok(()) = self.object_manager.unload_objects(&obj_layout, list) {
                        debug!(
                            "-> Success! Made Enough objects resident to allocate {} bytes in RAM",
                            obj_layout.size()
                        );

                        {
                            // unwrap is okay here because there are no other threads concurrently accessing it
                            // except from vnv_persist_all, but as it is guaranteed that no other threads run
                            // during its execution, it is fine
                            let guard = self.heap.try_lock().unwrap();
                            (
                                guard.as_mut().unwrap().allocate(obj_layout).expect(
                                    "unload_objects should made sure that there is enough space",
                                ),
                                guard,
                            )
                        }
                    } else {
                        warn!(
                            "-> Could not allocate an object with size {} in RAM",
                            obj_layout.size()
                        );

                        return Err(());
                    }
                }
            }
        };

        // metadata will be regarded dirty the moment the object is made persistently
        if self.remaining_dirty_size < ResidentObjectMetadata::fresh_object_dirty_size() {
            // deallocate here, because we need to drop guard here and we cant do this while
            // this object is allocated because of race conditions with vnv_persist_all
            guard.as_mut().unwrap().deallocate(obj_ptr, obj_layout);
            drop(guard);

            // not enough dirty bytes remaining
            // sync some data now by using object manager
            let required_bytes =
                ResidentObjectMetadata::fresh_object_dirty_size() - self.remaining_dirty_size;

            sync_dirty_data::<A, S, M>(
                &mut self.remaining_dirty_size,
                self.resident_list,
                &mut self.object_manager,
                required_bytes,
                storage,
                &self.heap,
                &mut self.resident_object_count,
            )?;

            // reallocate
            // this should not fail as we previously already allocated the space
            guard = self.heap.try_lock().unwrap();
            obj_ptr = guard.as_mut().unwrap().allocate(obj_layout).unwrap();
        }

        self.remaining_dirty_size -= ResidentObjectMetadata::fresh_object_dirty_size();

        // read data now and store it to the allocated region in memory
        let ptr = obj_ptr.as_ptr();

        let meta_ptr =
            ptr.add(offset_of!(ResidentObject<T>, metadata)) as *mut ResidentObjectMetadata;
        meta_ptr.write(ResidentObjectMetadata::new::<T>(alloc_id.offset));

        {
            // some checks and append to resident list
            let meta_ref = meta_ptr.as_mut().unwrap();

            debug_assert_eq!(
                ResidentObjectMetadata::fresh_object_dirty_size(),
                meta_ref.dirty_size(),
                "Dirty size of newly created metadata should match const value"
            );

            self.resident_list.push(meta_ref);
        }

        // FINISHED WITH CRITICAL ALLOCATE SECTION!
        drop(guard);

        {
            // read object data T
            let obj_data_ptr = ptr.add(offset_of!(ResidentObject<T>, data));
            let data_slice = slice_from_raw_parts_mut(obj_data_ptr, size_of::<T>())
                .as_mut()
                .unwrap();

            match storage.read(alloc_id.offset + calc_user_data_offset_static::<T>(), data_slice) {
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

                    guard.as_mut().unwrap().deallocate(obj_ptr, obj_layout);
                    drop(guard);

                    return Err(());
                }
            };
        }

        let obj_ref = (ptr as *mut ResidentObject<T>).as_mut().unwrap();

        self.resident_object_count += 1;
        Ok(obj_ref)
    }

    pub(crate) fn unload_object<T: Sized, S: PersistentStorageModule>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        storage: &mut S,
    ) -> Result<(), ()> {
        let mut iter = self.resident_list.iter_mut();
        while let Some(mut element) = iter.next() {
            if element.get_element().inner.offset == alloc_id.offset {
                // element found
                {
                    let element_ref = element.get_element();
                    if element_ref.inner.dirty_status.is_in_use() {
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
                    )
                    .expect("unloading should succeed")
                };
                self.resident_object_count -= 1;

                break;
            }
        }

        return Ok(());
    }

    pub(crate) fn try_to_allocate<T>(
        &mut self,
        data: T,
        offset: usize,
    ) -> Result<(), T> {
        let dirty_size = size_of::<T>() + ResidentObjectMetadata::fresh_object_dirty_size();
        if self.remaining_dirty_size < dirty_size {
            // should avoid syncing
            return Err(data);
        }

        let obj_layout = calc_resident_obj_layout_static::<T>();
        let guard = self.heap.try_lock().unwrap();
        let res_ptr = unsafe { guard.as_mut().unwrap().allocate(obj_layout) };
        let res_ptr = match res_ptr {
            Ok(res) => res,
            Err(_) => {
                return Err(data);
            }
        };

        self.remaining_dirty_size -= dirty_size;

        // read data now and store it to the allocated region in memory
        let ptr = res_ptr.as_ptr() as *mut ResidentObject<T>;

        let mut metadata = ResidentObjectMetadata::new::<T>(offset);
        metadata.inner.dirty_status.set_data_dirty(true);

        unsafe { ptr.write(ResidentObject { metadata, data }) };

        {
            // some checks and append to resident list
            let obj_ref = unsafe { ptr.as_mut().unwrap() };

            debug_assert_eq!(
                dirty_size,
                obj_ref.metadata.dirty_size(),
                "Previously calculated dirty size should match the current/actual one"
            );

            unsafe { self.resident_list.push(&mut obj_ref.metadata) };
        }

        drop(guard);
        self.resident_object_count += 1;

        Ok(())
    }

    /// Makes the object non resident.
    ///
    /// If `T` requires dropping, the object is made resident first and then dropped afterwards.
    pub(crate) fn drop<T: Sized, S: PersistentStorageModule>(
        &mut self,
        alloc_id: &AllocationIdentifier<T>,
        storage: &mut S,
    ) -> Result<(), ()> {
        self.check_integrity();
        if core::mem::needs_drop::<T>() {
            // require resident to drop object in memory
            let _ = unsafe { self.require_resident(alloc_id, storage) }?;
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

    pub(crate) fn is_resident<T>(&mut self, identifier: &AllocationIdentifier<T>) -> bool {
        unsafe { self.find_element_mut(identifier).is_some() }
    }

    pub(crate) unsafe fn get_mut<
        T: Sized,
        S: PersistentStorageModule,
    >(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        storage: &mut S,
    ) -> Result<*mut T, ()> {
        self.check_integrity();
        trace!("Get mutable reference (offset={})", identifier.offset);

        let obj_ref: *mut ResidentObject<T> =
            self.require_resident(identifier, storage)?;

        let bytes_to_sync = {
            let meta_ref = &mut obj_ref.as_mut().unwrap().metadata;

            // should be ensured by the rust compiler
            debug_assert!(
                !meta_ref.inner.dirty_status.is_in_use(),
                "This object should not be in use yet!"
            );
            debug_assert!(
                !meta_ref.inner.dirty_status.is_mutable_ref_active(),
                "This object should not have any mutable references!"
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
            sync_dirty_data::<A, S, M>(
                &mut self.remaining_dirty_size,
                self.resident_list,
                &mut self.object_manager,
                bytes_to_sync,
                storage,
                &self.heap,
                &mut self.resident_object_count,
            )?;
        }

        let obj_ref = obj_ref.as_mut().unwrap();
        let meta_ref = &mut obj_ref.metadata;

        if !meta_ref.inner.dirty_status.is_data_dirty() {
            // make dirty
            self.remaining_dirty_size -= meta_ref.inner.layout.size();
            meta_ref.inner.dirty_status.set_data_dirty(true);

            if !meta_ref.inner.dirty_status.is_general_metadata_dirty() {
                // because we update dirty status metadata has be dirty again
                if self.remaining_dirty_size
                    >= ResidentObjectMetadata::metadata_dirty_transition_size()
                {
                    // enough bytes remaining to make metadata dirty
                    // TODO this could be optimized by only making dirty status dirty (introduce new flag)
                    self.remaining_dirty_size -=
                        ResidentObjectMetadata::metadata_dirty_transition_size();
                    meta_ref.inner.dirty_status.set_general_metadata_dirty(true);
                } else {
                    // not enough bytes remaining to make metadata dirty
                    // flush dirty status now

                    // TODO: better way than unwrap?
                    // the problem here is that integrity of backup slot could be violated if this fails
                    ResidentObjectMetadataBackup::flush_dirty_status(
                        meta_ref.inner.offset,
                        &meta_ref.inner.dirty_status,
                        storage,
                    )
                    .unwrap();
                }
            }

            // set this again because of race conditions:
            // if vnv_persist_all is called after first set_data_dirty and before making metadata dirty
            // this would result in data_dirty = false
            meta_ref.inner.dirty_status.set_data_dirty(true);
        }

        meta_ref.inner.dirty_status.set_is_in_use(true);
        meta_ref.inner.dirty_status.set_is_mutable_ref_active(true);
        self.check_integrity();

        Ok(&mut obj_ref.data)
    }

    pub(crate) unsafe fn get_ref<
        T: Sized,
        S: PersistentStorageModule,
    >(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        storage: &mut S,
    ) -> Result<*const T, ()> {
        self.check_integrity();
        trace!("Get mutable reference (offset={})", identifier.offset);

        let obj_ref = self.require_resident(identifier, storage)?;
        let meta_ref = &mut obj_ref.metadata.inner;

        debug_assert!(
            !meta_ref.dirty_status.is_in_use(),
            "This object should not be in use yet!"
        );
        debug_assert!(
            !meta_ref.dirty_status.is_mutable_ref_active(),
            "This object should not have any mutable references!"
        );

        meta_ref.dirty_status.set_is_in_use(true);

        Ok(&mut obj_ref.data)
    }

    pub(crate) unsafe fn release_mut<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) {
        self.check_integrity();
        trace!("Release mutable reference (offset={})", identifier.offset);
        if let Some(meta_ptr) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
            let meta_ref = &mut meta_ref.inner;
            debug_assert!(meta_ref.dirty_status.is_in_use());
            debug_assert!(meta_ref.dirty_status.is_mutable_ref_active());

            meta_ref.dirty_status.set_is_in_use(false);
            meta_ref.dirty_status.set_is_mutable_ref_active(false);
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
            debug_assert!(meta_ref.dirty_status.is_in_use());
            debug_assert!(!meta_ref.dirty_status.is_mutable_ref_active());

            meta_ref.dirty_status.set_is_in_use(false);
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
            self._initial_dirty_size,
            "did not match initial dirty size: curr_dirty_size: {}, remaining_dirty_size: {}",
            dirty_size,
            self.remaining_dirty_size
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
    resident_object_count: &'a mut usize,
) -> Result<(), ()> {
    if required_bytes == 0 {
        return Ok(());
    }

    let prev_dirty_size = *remaining_dirty_size;

    let mut args = DirtyItemListArguments {
        remaining_dirty_size,
        storage: storage,
        allocator,
        resident_object_count,
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
