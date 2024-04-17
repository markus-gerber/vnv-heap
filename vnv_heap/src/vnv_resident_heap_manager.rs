use core::{alloc::Layout, ptr::{null_mut, NonNull}};

use log::{debug, warn};

use crate::{
    allocation_options::AllocationOptions, modules::{allocator::AllocatorModule, page_storage::PageStorageModule}, util::{bit_array::BitArray, ceil_div, get_page_size}, vnv_heap_metadata::VNVHeapMetadata, vnv_resident_heap::{aligned_alloc_module_offset, VNVResidentHeap}
};

/// Calculates the upper bound on how many heaps can be resident at one time
///
/// `resident_size` is given in size in bytes
pub(crate) fn calc_max_resident_heap_count(resident_size: usize) -> usize {
    ceil_div(resident_size, get_page_size())
}

pub struct VNVResidentHeapManagerConfig {
    pub max_resident_size: usize,
    pub max_dirty_size: usize,
}

// expose item type of `heaps` to outside for calculating offsets and sizes
pub(crate) type VNVResidentHeapListItem<A> = Option<VNVResidentHeap<A>>;

pub(crate) struct VNVResidentHeapManager<'a, A: AllocatorModule + 'static> {
    heaps: &'a mut [VNVResidentHeapListItem<A>],
    recover_heaps: BitArray<'a>,

    /// Amount of bytes that is left
    remaining_resident_size: usize,

    /// Amount of bytes that can be used for dirty pages
    remaining_dirty_size: usize,
}

impl<'a, A: AllocatorModule> VNVResidentHeapManager<'a, A> {
    pub(crate) fn new(
        heaps: &'a mut [VNVResidentHeapListItem<A>],
        recover_heaps: BitArray<'a>,
        config: VNVResidentHeapManagerConfig,
    ) -> Self {
        debug_assert!(
            heaps.len() <= recover_heaps.len(),
            "recover_heaps should not be smaller than heaps array"
        );

        debug_assert!(heaps.iter().all(|x| x.is_none()), "all items should be none");

        Self {
            heaps,
            recover_heaps,
            remaining_resident_size: config.max_resident_size,
            remaining_dirty_size: config.max_dirty_size,
        }
    }
}

impl<A: AllocatorModule> VNVResidentHeapManager<'_, A> {
    /// Makes a heap resident.
    /// If `init` is true, the heap will be initialized. (All other data will be overwritten!)
    ///
    /// Note that this function requires the heap not be resident yet (for performance reasons).
    ///
    /// ### Safety
    ///
    /// Only set `init` to true once at the first call.
    pub(crate) unsafe fn map_heap<S: PageStorageModule>(
        &mut self,
        metadata: &mut VNVHeapMetadata<A>,
        init: bool,
        page_storage: &mut S,
    ) -> &mut VNVResidentHeap<A> {
        // test if heap is already resident
        debug_assert!(
            !self.heaps.iter().any(|item| {
                match item {
                    Some(item) => item.get_metadata_ptr() == metadata,
                    None => false,
                }
            }),
            "Heap should not be resident yet"
        );

        log::trace!("Mapping heap at offset {}", metadata.offset);

        self.require_remaining_resident_size(metadata.size, page_storage);

        // map heap and get pointer of AllocatorModule
        let base_ptr = unsafe { page_storage.map(metadata.offset, metadata.size) }
            .unwrap()
            .as_ptr() as *mut u8;

        if init {
            let (heap_ptr, heap_size) = unsafe {
                // create new allocator module
                (base_ptr as *mut A).write(A::new());

                // calculate remaining space that should be used by the allocator module
                let offset = aligned_alloc_module_offset::<A>();
                (base_ptr.offset(offset as isize), metadata.size - offset)
            };

            // initialize allocator module to mapped space
            let max_alloc_size = unsafe {
                (base_ptr as *mut A)
                    .as_mut()
                    .unwrap()
                    .init(heap_ptr, heap_size)
            };

            metadata.max_size_hint = max_alloc_size;
        }

        let ptr = base_ptr as *mut A;

        // cast pointer to reference
        let heap_ref = unsafe { ptr.as_mut() }.unwrap();

        let heap: VNVResidentHeap<A> = VNVResidentHeap::new(heap_ref, metadata);

        // store into list
        for item in self.heaps.iter_mut() {
            if item.is_none() {
                *item = Some(heap);

                if let Some(data_ref) = item {
                    // add pointer to list item
                    metadata.resident_ptr = data_ref;

                    return data_ref;
                } else {
                    panic!("item should be not None!");
                }
            }
        }

        // should never happen, as list should always be large enough
        panic!("No free slot for new NVNResidentHeap object found!");
    }

    pub(crate) unsafe fn allocate<T>(&mut self, heap: *mut VNVResidentHeap<A>, options: AllocationOptions<T>) -> Result<(usize, *mut VNVHeapMetadata<A>), AllocationOptions<T>> {
        let heap_ref = heap.as_mut().unwrap();
        let offset = heap_ref.allocate(options)?;

        Ok((offset, heap_ref.get_metadata_ptr()))
    }

    pub(crate) unsafe fn deallocate<T>(&mut self, heap: *mut VNVResidentHeap<A>, offset: usize, layout: &Layout) {
        heap.as_mut().unwrap().deallocate(offset, &layout)
    }

    /// Gets an immutable reference.
    /// It will be ensure that this heap will be resident until this reference is dropped.
    ///
    /// ### Safety
    ///
    /// `offset` has to be valid:
    /// - Should not be greater than the size of this heap
    /// - Should point to valid and aligned data of type `T`
    pub(crate) unsafe fn get_ref<'b, T>(
        &mut self,
        heap: *mut VNVResidentHeap<A>,
        offset: usize,
    ) -> &'b T {
        let heap_ref = heap.as_mut().unwrap();

        heap_ref.get_ref(offset)
    }

    /// Gets a mutable reference.
    /// The heap will be resident until this reference is dropped.
    ///
    /// ### Safety
    ///
    /// `offset` has to be valid:
    /// - Should not be greater than the size of this heap
    /// - Should point to valid and aligned data of type `T`
    pub(crate) unsafe fn get_mut<'b, T, P: PageStorageModule>(
        &mut self,
        heap: *mut VNVResidentHeap<A>,
        offset: usize,
        page_storage: &mut P,
    ) -> &'b mut T {
        let heap_ref = heap.as_mut().unwrap();
        let size = heap_ref.get_metadata_ptr().as_mut().unwrap().size;

        let heap_ref = if !heap_ref.is_dirty() {
            // this heap is not dirty
            // make sure that remaining_dirty_size is big enough

            // forget this reference to avoid two mutable references at one time
            // because require_remaining_dirty_size will iterate through the array
            drop(heap_ref);

            self.require_remaining_dirty_size(size, page_storage);

            heap.as_mut().unwrap()
        } else {
            // do nothing: this heap is already dirty
            heap_ref
        };

        heap_ref.get_mut(offset)
    }

    /// Releases a reference
    ///
    /// ### Safety
    ///
    /// Calling this function twice with the same reference will break this heap!
    pub(crate) unsafe fn release_ref<T>(
        &mut self,
        heap: *mut VNVResidentHeap<A>,
        t: &T,
    ) {
        let heap_ref = heap.as_mut().unwrap();
        heap_ref.release_ref(t);
    }

    /// Releases a mutable reference
    ///
    /// ### Safety
    ///
    /// Calling this function twice with the same reference will break this heap!
    pub(crate) unsafe fn release_mut<T>(
        &mut self,
        heap: *mut VNVResidentHeap<A>,
        t: &mut T,
    ) {
        let heap_ref = heap.as_mut().unwrap();
        heap_ref.release_mut(t);
    }

    pub(crate) unsafe fn decrease_resident_size(&mut self, size: usize) {

    }

    pub(crate) unsafe fn decrease_dirty_size(&mut self, size: usize) {

    }

    /// Requires that at least `size` bytes are ready to get dirty.
    ///
    /// If necessary, changes will be flushed until this condition is met.
    fn require_remaining_dirty_size<S: PageStorageModule>(
        &mut self,
        size: usize,
        page_storage: &mut S,
    ) {
        if size > self.remaining_dirty_size {
            let mut to_flush = size - self.remaining_dirty_size;
            debug!(
                "Need to flush {} bytes (required {} new dirty bytes)",
                to_flush, size
            );

            for item in self.heaps.iter_mut() {
                if let Some(item) = item {
                    // flushing makes no sense for heaps that have mutable references
                    if item.is_dirty() && !item.has_mutable_references() {
                        let size = unsafe { item.get_metadata_ptr().as_ref().unwrap().size };

                        VNVResidentHeapManager::sync_heap(item, page_storage, &mut self.remaining_dirty_size);

                        if size >= to_flush {
                            // finished flushing
                            return;
                        }

                        to_flush -= size;
                    }
                }
            }

            panic!("Could not flush enough pages! (Too much open mutable references...)");
        }
    }

    /// Requires that at least `size` bytes are ready to get persistent.
    ///
    /// If necessary, resident heaps be unmapped until this condition is met.
    fn require_remaining_resident_size<S: PageStorageModule>(
        &mut self,
        size: usize,
        page_storage: &mut S,
    ) {
        if size > self.remaining_resident_size {
            let mut to_store = size - self.remaining_resident_size;
            debug!(
                "Need to store {} bytes (required {} new resident bytes)",
                to_store, size
            );

            for item in self.heaps.iter_mut() {
                if let Some(heap) = item {
                    // flushing makes no sense for heaps that have mutable references
                    if !heap.has_mutable_references() && !heap.has_immutable_references() {
                        let size = unsafe { heap.get_metadata_ptr().as_ref().unwrap().size };

                        // unmap heap and sync all pending changes
                        unsafe { VNVResidentHeapManager::unmap_heap(item, page_storage, &mut self.remaining_resident_size, &mut self.remaining_dirty_size) };

                        if size >= to_store {
                            // finished flushing
                            return;
                        }

                        to_store -= size;
                    }
                }
            }

            panic!("Could not store enough pages! (Too much open (im)mutable references...)");
        }
    }

    /// Helper function: Writes all changes back to non volatile storage
    ///
    /// Does nothing if there are no changes to sync (heap is not dirty)
    fn sync_heap<S: PageStorageModule>(
        item: &mut VNVResidentHeap<A>,
        page_storage: &mut S,
        remaining_dirty_size: &mut usize
    ) {
        if !item.is_dirty() {
            // nothing to sync
            return;
        }

        let size = unsafe { item.get_metadata_ptr().as_ref().unwrap().size };
        let ptr = NonNull::new((item.get_heap_ref() as *mut A) as *mut u8).unwrap();

        page_storage.persist(ptr, size).unwrap();

        *remaining_dirty_size -= size;
    }


    /// Helper function: Unmaps a given resident heap:
    ///
    /// - It will be deleted from `self.heaps`
    /// - All the necessary pages will be unmapped
    /// - `remaining_resident_size` and `remaining_dirty_size` will be updated
    ///
    /// ### Safety
    ///
    /// There should not be any mutable/immutable references to `item` left!
    unsafe fn unmap_heap<S: PageStorageModule>(
        item: &mut VNVResidentHeapListItem<A>,
        page_storage: &mut S,
        remaining_resident_size: &mut usize,
        remaining_dirty_size: &mut usize
    ) {
        let obj_ref = match item {
            Some(x) => x,
            None => {
                return;
            }
        };

        debug_assert!(
            !obj_ref.has_immutable_references(),
            "There should not be any immutable references to this heap left!"
        );
        debug_assert!(
            !obj_ref.has_mutable_references(),
            "There should not be any mutable references to this heap left!"
        );

        log::trace!(
            "Unmapping heap at offset {}",
            obj_ref.get_metadata_ptr().as_ref().unwrap().offset
        );

        // sync any pending changes
        VNVResidentHeapManager::sync_heap(obj_ref, page_storage, remaining_dirty_size);

        let ptr = NonNull::new((obj_ref.get_heap_ref() as *mut A) as *mut u8).unwrap();
        let size = obj_ref.get_metadata_ptr().as_ref().unwrap().size;

        // remove pointer to our obj_ref
        let metadata_ref = obj_ref.get_metadata_ptr().as_mut().unwrap();
        metadata_ref.resident_ptr = null_mut();

        // remove item from list
        *item = None;

        // finally unmap page(s)
        page_storage.unmap(ptr, size).unwrap();

        // release resident size
        *remaining_resident_size -= size;
    }

    pub(crate) fn before_drop<S: PageStorageModule>(&mut self, page_storage: &mut S) {
        for heap in self.heaps.iter_mut() {
            if let Some(val) = heap {
                if val.has_immutable_references() || val.has_mutable_references() {
                    warn!("before_drop: There are still some (im)mutable references left!");
                }
            }

            unsafe { VNVResidentHeapManager::unmap_heap(heap, page_storage, &mut self.remaining_resident_size, &mut self.remaining_dirty_size) };
        }
    }
}
