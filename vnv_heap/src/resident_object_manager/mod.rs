use core::alloc::Layout;
use core::{mem::size_of, ptr::NonNull};

use log::{debug, error, trace, warn};

use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule,
        nonresident_allocator::{CountedNonResidentLinkedList, NonResidentAllocatorModule},
        persistent_storage::PersistentStorageModule,
    },
    util::multi_linked_list::{DeleteHandle, MultiLinkedList},
};

use self::resident_object::{
    calc_resident_obj_layout, ResidentObject, ResidentObjectMetadata, ResidentObjectMetadataInner,
};

mod resident_object;

#[cfg(test)]
mod test;

/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
struct ResidentObjectMetadataBackup {
    /// size of the object
    size: usize,

    /// where is this objects stored inside of
    /// persistent storage
    offset: usize,

    /// how many references are there
    ref_cnt: usize,

    /// at which address does this data live
    /// (pointers could exist here so we need to restore
    /// the object at exactly the previous address)
    resident_ptr: usize,
}

impl ResidentObjectMetadataBackup {
    fn new_unused() -> Self {
        ResidentObjectMetadataBackup {
            size: 0,
            offset: 0,
            ref_cnt: 0,
            resident_ptr: 0,
        }
    }

    fn is_unused(&self) -> bool {
        self.resident_ptr == 0
    }
}

pub(crate) struct ResidentObjectManager<A: AllocatorModule> {
    /// In memory heap for resident objects and their metadata
    heap: A,

    /// Allocated list inside non volatile storage
    /// to backup metadata when PFI occurs
    ///
    /// Following is always true: `resident_object_meta_backup.len() >= resident_object_count`
    resident_object_meta_backup: CountedNonResidentLinkedList<ResidentObjectMetadataBackup>,

    /// How many objects are currently resident?
    resident_object_count: usize,

    resident_list: MultiLinkedList<
        ResidentObjectMetadata,
        ResidentObjectMetadataInner,
        fn(*mut ResidentObjectMetadata) -> *mut *mut ResidentObjectMetadata,
        fn(*mut ResidentObjectMetadata) -> *mut ResidentObjectMetadataInner,
    >,
    dirty_list: MultiLinkedList<
        ResidentObjectMetadata,
        ResidentObjectMetadataInner,
        fn(*mut ResidentObjectMetadata) -> *mut *mut ResidentObjectMetadata,
        fn(*mut ResidentObjectMetadata) -> *mut ResidentObjectMetadataInner,
    >,

    /// How many bytes can still be made dirty without
    /// violating users requirements
    remaining_dirty_size: usize,
}

impl<A: AllocatorModule> ResidentObjectManager<A> {
    /// Create a new resident object manager
    ///
    /// **Note**: Will overwrite any data, at index 0 of the given persistent storage.
    ///
    /// Returns the newly created instance and the offset from which on data can
    /// be stored to persistent storage safely again.
    pub(crate) fn new<S: PersistentStorageModule>(
        resident_buffer: &mut [u8],
        max_dirty_size: usize,
        storage: &mut S,
    ) -> Result<(Self, usize), ()> {
        let mut heap = A::new();
        unsafe {
            let start_ref = &mut resident_buffer[0];
            heap.init(start_ref, resident_buffer.len());
        }

        // backup item has to be the first in the persistent storage, so restoring is easier
        let mut meta_backup_list = CountedNonResidentLinkedList::new();
        unsafe { meta_backup_list.push(0, ResidentObjectMetadataBackup::new_unused(), storage) }?;
        let offset =
            CountedNonResidentLinkedList::<ResidentObjectMetadataBackup>::total_item_size();

        let instance = ResidentObjectManager {
            heap,
            resident_object_meta_backup: meta_backup_list,
            resident_list: unsafe {
                MultiLinkedList::new(
                    ResidentObjectMetadata::get_next_resident_item,
                    ResidentObjectMetadata::get_inner,
                )
            },
            dirty_list: unsafe {
                MultiLinkedList::new(
                    ResidentObjectMetadata::get_next_dirty_item,
                    ResidentObjectMetadata::get_inner,
                )
            },
            resident_object_count: 0,
            remaining_dirty_size: max_dirty_size,
        };

        Ok((instance, offset))
    }
}

/// The amount of bytes that will be counted as dirty for each `ResidentObjectMetadata`
///
/// It is set to the size of `ResidentObjectMetadataBackup` because that is the size of the metadata
/// part that will saved to persistent storage (some fields of `ResidentObjectMetadata` can be reconstructed)
const METADATA_DIRTY_SIZE: usize = size_of::<ResidentObjectMetadataBackup>();

impl<A: AllocatorModule> ResidentObjectManager<A> {
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
    ) -> Result<
        (
            *mut ResidentObjectMetadata,
            *mut ResidentObjectMetadataInner,
            *mut T,
        ),
        (),
    > {
        if let Some((container_ptr, meta_ptr)) = self.find_element_mut(&alloc_id) {
            // already resident
            let res_object_ptr =
                ResidentObjectMetadata::metadata_to_resident_obj_ptr(container_ptr);
            return Ok((
                container_ptr,
                meta_ptr,
                ResidentObject::to_data_ptr(res_object_ptr),
            ));
        }

        trace!("Make object resident (offset: {})", alloc_id.offset);

        debug_assert!(
            self.resident_object_count <= self.resident_object_meta_backup.len(),
            "requirement should not be violated (resident_object_count={}, resident_object_meta_backup.len()={})", self.resident_object_count, self.resident_object_meta_backup.len()
        );
        if self.resident_object_count == self.resident_object_meta_backup.len() {
            // acquire new slot for backup
            let ptr = non_resident_allocator.allocate(
                CountedNonResidentLinkedList::<ResidentObjectMetadataBackup>::item_layout(),
                storage,
            )?;

            self.resident_object_meta_backup.push(
                ptr,
                ResidentObjectMetadataBackup::new_unused(),
                storage,
            )?;
        }

        let obj_ptr = self.allocate_resident_space(Layout::new::<ResidentObject<T>>(), storage)?;

        // metadata will be regarded dirty the moment the object is made persistently
        if self.remaining_dirty_size < METADATA_DIRTY_SIZE {
            self.sync_dirty_data(METADATA_DIRTY_SIZE, storage)?;
            debug_assert!(
                self.remaining_dirty_size >= METADATA_DIRTY_SIZE,
                "should have made enough space"
            );
        }
        self.remaining_dirty_size -= METADATA_DIRTY_SIZE;

        // read data now and store it to the allocated region in memory
        let obj_ptr = obj_ptr.as_ptr() as *mut ResidentObject<T>;

        let data: T = storage.read_data(alloc_id.offset)?;
        obj_ptr.write(ResidentObject {
            data,
            metadata: ResidentObjectMetadata::new::<T>(alloc_id.offset),
        });

        let meta_ref = (obj_ptr as *mut ResidentObjectMetadata).as_mut().unwrap();
        self.resident_list.push(meta_ref);

        let data_ptr = ResidentObject::to_data_ptr(
            ResidentObjectMetadata::metadata_to_resident_obj_ptr(meta_ref),
        );

        self.resident_object_count += 1;
        Ok((meta_ref, &mut meta_ref.inner, data_ptr))
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
        if std::mem::needs_drop::<T>() {
            // require resident to drop object in memory
            let _ = unsafe { self.require_resident(alloc_id, non_resident_allocator, storage) }?;
        }

        let mut iter_mut = self.resident_list.iter_mut();
        while let Some(mut curr) = iter_mut.next() {
            // important: drop the item reference here
            // so we can iterate over the dirty list later
            // (without having two mutable references to the same data)
            let (found_item, is_dirty) = unsafe {
                let item_ref = curr.get_element();

                if item_ref.offset == alloc_id.offset {
                    // should be ensured by the rust compiler
                    debug_assert_eq!(
                        item_ref.ref_cnt, 0,
                        "There should be no references to this object anymore!"
                    );
                }

                (item_ref.offset == alloc_id.offset, item_ref.is_dirty)
            };

            if found_item {
                // item was found
                if is_dirty {
                    // item is dirty
                    // as it should be dropped: does not need to by synced
                    // however we need to remove it from the dirty list
                    let mut dirty_iter_mut = self.dirty_list.iter_mut();

                    // for debugging: check if item was found
                    #[cfg(debug_assertions)]
                    let mut found = false;

                    while let Some(mut curr_dirty) = dirty_iter_mut.next() {
                        if unsafe { curr_dirty.get_element().offset } == alloc_id.offset {
                            curr_dirty.delete();
                            found = true;
                            break;
                        }
                    }

                    debug_assert!(
                        found,
                        "Item should be in the dirty list (as its dirty flag was set to true)"
                    );
                    self.remaining_dirty_size += size_of::<T>();
                }

                let ptr: *mut ResidentObjectMetadataInner = unsafe { curr.get_element() };
                curr.delete();

                unsafe {
                    // drop whole object (including T)
                    let obj_ptr = ResidentObjectMetadataInner::ptr_to_resident_obj_ptr::<T>(ptr);
                    obj_ptr.drop_in_place();
                    self.heap.deallocate(
                        NonNull::new(obj_ptr as *mut u8).unwrap(),
                        Layout::new::<ResidentObject<T>>(),
                    )
                };

                // update managers metadata
                self.remaining_dirty_size += METADATA_DIRTY_SIZE;
                self.resident_object_count -= 1;

                return Ok(());
            }
        }

        // if this point is reached
        // it means that this object was not resident

        if std::mem::needs_drop::<T>() {
            // should not happen: object should be made resident and dropped in RAM
            debug_assert!(false, "Should not happen");
            Err(())
        } else {
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
        trace!("Get mutable reference (offset={})", identifier.offset);

        let (container_ptr, _, obj_ptr) =
            self.require_resident(identifier, non_resident_allocator, storage)?;
        let container_ref = container_ptr.as_mut().unwrap();

        // should be ensured by the rust compiler
        debug_assert_eq!(
            container_ref.inner.ref_cnt, 0,
            "There should be no references to this object yet!"
        );

        if !container_ref.inner.is_dirty {
            // was previously not dirty
            if self.remaining_dirty_size < container_ref.inner.layout.size() {
                // not enough space left to make it dirty
                // sync other data now
                self.sync_dirty_data(container_ref.inner.layout.size(), storage)?;
                debug_assert!(
                    self.remaining_dirty_size >= container_ref.inner.layout.size(),
                    "should have made enough space"
                );
            }

            self.remaining_dirty_size -= container_ref.inner.layout.size();
            self.dirty_list.push(container_ref);
        }

        container_ref.inner.is_dirty = true;
        container_ref.inner.ref_cnt = usize::MAX;

        Ok(obj_ptr)
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
        trace!("Get mutable reference (offset={})", identifier.offset);

        let (_, meta_ptr, obj_ptr) =
            self.require_resident(identifier, non_resident_allocator, storage)?;
        let meta_ref = meta_ptr.as_mut().unwrap();

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
            return Err(());
        }

        meta_ref.ref_cnt += 1;

        Ok(obj_ptr)
    }

    pub(crate) unsafe fn release_mut<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        _data: &mut T,
    ) {
        trace!("Release mutable reference (offset={})", identifier.offset);
        if let Some((_, meta_ptr)) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
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
    }

    pub(crate) unsafe fn release_ref<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        _data: &T,
    ) {
        trace!("Release immutable reference (offset={})", identifier.offset);
        if let Some((_, meta_ptr)) = self.find_element_mut(identifier) {
            let meta_ref = meta_ptr.as_mut().unwrap();
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
        if self.remaining_dirty_size >= required_bytes {
            return Ok(());
        }

        let mut iter = self.dirty_list.iter_mut();
        while let Some(mut curr_item) = iter.next() {
            let meta_ref = curr_item.get_element();
            if meta_ref.is_dirty && meta_ref.ref_cnt == 0 {
                let _ = sync_object_dynamic(curr_item, &mut self.remaining_dirty_size, storage);
                if self.remaining_dirty_size >= required_bytes {
                    return Ok(());
                }
            }
        }

        // could not sync enough data
        debug_assert!(self.remaining_dirty_size < required_bytes);
        error!(
            "Could not sync enough data (remaining_dirty_size: {}, required_bytes: {})",
            self.remaining_dirty_size, required_bytes
        );
        Err(())
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

        // does not have enough space to allocate new object
        // try to make other objects non resident

        let mut iter = self.resident_list.iter_mut();
        while let Some(mut item) = iter.next() {
            let item_ref = item.get_element();

            trace!("Try to make object nonresident {}", item_ref.ref_cnt);
            if item_ref.ref_cnt == 0 {
                let res = make_object_nonresident_dynamic(
                    item,
                    &mut self.dirty_list,
                    &mut self.heap,
                    &mut self.remaining_dirty_size,
                    &mut self.resident_object_count,
                    storage,
                );
                if res.is_ok() {
                    // made object nonresident, try to allocate now
                    match self.heap.allocate(layout) {
                        Ok(res) => {
                            debug!("-> Success! Made Enough objects resident to allocate {} bytes in RAM", layout.size());
                            return Ok(res);
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        warn!(
            "-> Could not allocate an object with size {} in RAM",
            layout.size()
        );
        Err(())
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
    ) -> Option<(
        *mut ResidentObjectMetadata,
        *mut ResidentObjectMetadataInner,
    )> {
        let mut iter = self.resident_list.iter_mut();
        while let Some(mut item) = iter.next() {
            let item_ref = item.get_element();
            if item_ref.offset == alloc_id.offset {
                return Some((
                    ResidentObjectMetadata::ptr_from_meta_inner_mut(item_ref),
                    item_ref,
                ));
            }
        }
        None
    }
}

/// Tries to sync an object that is known to be dirty
/// and one on which there are no references left
#[inline]
unsafe fn sync_object_dynamic<S: PersistentStorageModule>(
    mut curr_item: DeleteHandle<
        ResidentObjectMetadata,
        ResidentObjectMetadataInner,
        fn(*mut ResidentObjectMetadata) -> *mut *mut ResidentObjectMetadata,
        fn(*mut ResidentObjectMetadata) -> *mut ResidentObjectMetadataInner,
    >,
    remaining_dirty_size: &mut usize,
    storage: &mut S,
) -> Result<(), ()> {
    let meta_ref = curr_item.get_element();
    debug_assert!(meta_ref.is_dirty);
    debug_assert!(meta_ref.ref_cnt == 0);

    let data_range = meta_ref.dynamic_metadata_to_data_range();

    let res = storage.write(meta_ref.offset, data_range);
    if res.is_ok() {
        // give back space and remove from dirty list
        *remaining_dirty_size += meta_ref.layout.size();
        curr_item.delete();
    }
    res
}

/// Synchronizes any changes back to persistent storage.
///
/// - Returns `Ok(true)` if the item was found, synced and removed from the dirty list.
/// - Returns `Ok(false)` if the item was not found
/// - Returns `Err(())` if there was an error while syncing the data
///
/// ### Safety
///
/// As this list modifies the `dirty_list`, make sure there are no open (mutable) references
#[inline]
unsafe fn sync_object<T: Sized, S: PersistentStorageModule>(
    dirty_list: &mut MultiLinkedList<
        ResidentObjectMetadata,
        ResidentObjectMetadataInner,
        fn(*mut ResidentObjectMetadata) -> *mut *mut ResidentObjectMetadata,
        fn(*mut ResidentObjectMetadata) -> *mut ResidentObjectMetadataInner,
    >,
    storage: &mut S,
    offset: usize,
    remaining_dirty_size: &mut usize,
) -> Result<(), ()> {
    let mut iterator = dirty_list.iter_mut();
    while let Some(mut curr_item) = iterator.next() {
        let metadata = curr_item.get_element();
        if metadata.offset == offset {
            if metadata.ref_cnt != 0 {
                error!("Cannot sync object at offset {}, because there are open references left (ref_cnt = {})", metadata.offset, metadata.ref_cnt);
                debug_assert!(false, "Cannot sync object at offset {}, because there are open references left (ref_cnt = {})", metadata.offset, metadata.ref_cnt);
                return Err(());
            }
            let data_ptr: *const T = ResidentObject::to_data_ptr(metadata.to_resident_obj_ptr());
            let res = storage.write_data(metadata.offset, data_ptr.as_ref().unwrap());
            return match res {
                Ok(_) => {
                    // give back space and remove from dirty list
                    *remaining_dirty_size += metadata.layout.size();
                    curr_item.delete();
                    Ok(())
                }
                Err(_) => Err(()),
            };
        }
    }
    Ok(())
}

/// This function is used to make an object nonresident even
/// when we don't know the type T of this item
///
/// ### Safety
///
/// As this list modifies the `dirty_list` and `resident_list`, make sure there are no open (mutable) references
#[inline]
unsafe fn make_object_nonresident_dynamic<A: AllocatorModule, S: PersistentStorageModule>(
    mut delete_handle: DeleteHandle<
        ResidentObjectMetadata,
        ResidentObjectMetadataInner,
        fn(*mut ResidentObjectMetadata) -> *mut *mut ResidentObjectMetadata,
        fn(*mut ResidentObjectMetadata) -> *mut ResidentObjectMetadataInner,
    >,
    dirty_list: &mut MultiLinkedList<
        ResidentObjectMetadata,
        ResidentObjectMetadataInner,
        fn(*mut ResidentObjectMetadata) -> *mut *mut ResidentObjectMetadata,
        fn(*mut ResidentObjectMetadata) -> *mut ResidentObjectMetadataInner,
    >,
    heap: &mut A,
    remaining_dirty_size: &mut usize,
    resident_object_count: &mut usize,
    storage: &mut S,
) -> Result<(), ()> {
    let (is_dirty, offset) = {
        let meta_ref = delete_handle.get_element();
        (meta_ref.is_dirty, meta_ref.offset)
    };

    // item was found
    if is_dirty {
        // item is dirty, so iterate over dirty list
        // sync item, and remove it from list
        let mut dirty_iter = dirty_list.iter_mut();
        while let Some(mut curr) = dirty_iter.next() {
            if curr.get_element().offset == offset {
                sync_object_dynamic(curr, remaining_dirty_size, storage)?;
                break;
            }
        }

        // item was successfully synced now
        // continue with removing object from RAM
    }

    let (ptr, layout) = unsafe {
        (
            delete_handle.get_container_ptr(),
            delete_handle.get_element().layout.clone(),
        )
    };
    delete_handle.delete();

    // drop ResidentObjectMetadata (! - because objects should only be dropped when
    // the corresponding VNVObject gets dropped and not when an object is not resident anymore)
    // and deallocate memory afterwards
    unsafe {
        ptr.drop_in_place();
        heap.deallocate(
            NonNull::new(ptr as *mut u8).unwrap(),
            calc_resident_obj_layout(layout),
        )
    };

    // update managers metadata
    *remaining_dirty_size += METADATA_DIRTY_SIZE;
    *resident_object_count -= 1;

    trace!("Made object nonresident with offset {}", offset);

    Ok(())
}
