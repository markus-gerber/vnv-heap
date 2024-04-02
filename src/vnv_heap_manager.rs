use std::{alloc::Layout, mem::size_of, ptr::{null_mut, NonNull}, sync::atomic::{AtomicU16, Ordering}};
use log::debug;

use crate::{allocation_options::AllocationOptions, modules::{allocator::AllocatorModule, page_storage::PageStorageModule}};

/// Converts the identifying offset to a in ram ptr.
unsafe fn offset_to_ptr<A: AllocatorModule>(heap: &mut A, offset: usize) -> *mut u8 {
    ((heap as *mut A) as *mut u8).offset(offset as isize)
}

/// Converts ptr in ram to object identifying offset.
/// 
/// This includes from the actual start of the heap, so `offset=0` would point to the heap's metadata if it has any.
/// Its not the best style tho, but it would be more complicated to figure out where actual user data starts.
unsafe fn ptr_to_offset<A: AllocatorModule>(heap: &mut A, ptr: *mut u8) -> usize {
    let offset = ptr.offset_from((heap as *mut A) as *mut u8);
    debug_assert!(offset >= 0, "offset has to be positive");

    return offset as usize;
}

fn aligned_alloc_module_offset<A: AllocatorModule>() -> usize {
    let layout = Layout::new::<A>();
    // TODO is this correct?
    let aligned_layout = layout.align_to(size_of::<usize>()).unwrap();
    let padded_layout = aligned_layout.pad_to_align();

    padded_layout.size()
}

/// Calculates the minimum size of a new heap in pages containing `layout` as multiples of `page_size`.
/// Includes that fact that the allocator module `A` is stored at the start of the new pages.
pub(crate) fn calc_min_pages_of_new_heap<A: AllocatorModule>(page_size: usize, layout: &Layout) -> usize {
    let module_offset = aligned_alloc_module_offset::<A>();
    let additional_size = A::calc_min_size_for_layout(layout);

    let total_size = module_offset + additional_size;
    let pages_ceil = total_size / page_size;

    if pages_ceil * page_size == total_size {
        pages_ceil
    } else {
        pages_ceil + 1
    }
}

/// Manages one heap and keeps additional metadata about it.
/// 
/// Heaps don't need to be resident, but this manager handles dynamic mapping and unmapping of heaps
pub(crate) struct VNVHeapManager<A: AllocatorModule + Sized> {
    /// count of immutable references to this heap
    ref_count: AtomicU16,

    /// count of mutable references to this heap
    mut_count: AtomicU16,

    /// A size hint that hints
    /// the maximum size that still can be allocated.
    /// 
    /// Because alignment is not considered, allocation can fail even if `max_size_hint >= size_to_alloc`.
    /// But if `max_size_hint < size_to_alloc` you can be sure that there is no space left.
    /// So this means `max_size_hint` is just an **upper limit** for allocation. 
    max_size_hint: usize,

    /// is this heap dirty? (are there unsynced changes)
    dirty: bool,

    /// offset of this data in the heap storage
    offset: u64,

    /// size of the total heap, including the metadata
    size: usize,

    /// pointer to allocator module, is null if not resident
    heap_ptr: *mut A,
}

impl<A: AllocatorModule> VNVHeapManager<A> {
    /// initializes the heap metadata
    pub(crate) fn new<P: PageStorageModule>(offset: u64, size: usize, page_storage: &mut P) -> Self {
        let map_res = unsafe { 
            page_storage.map(offset, size)
        }.unwrap();
        let base_ptr = map_res.as_ptr() as *mut u8;

        let (heap_ptr, heap_size) = unsafe {
            (base_ptr as *mut A).write(A::new());
            let offset = aligned_alloc_module_offset::<A>();
            (base_ptr.offset(offset as isize), size - offset)
        };

        // initialize Allocator module to mmaped space
        let max_alloc_size = unsafe {
            (base_ptr as *mut A).as_mut().unwrap().init(heap_ptr, heap_size)
        };

        VNVHeapManager {
            ref_count: 0.into(),
            mut_count: 0.into(),
            max_size_hint: max_alloc_size,
            dirty: true,
            offset: offset,
            size: size,
            heap_ptr: base_ptr as *mut A,
        }
    }

    /// Checks if this heap has any space left to allocate `layout`
    /// 
    /// **Note**: Currently alignment is not considered, so in some cases `has_space_left` could return `true`
    // even though `allocate` will fail
    pub(crate) fn has_space_left(&self, layout: &Layout) -> bool {
        self.max_size_hint >= layout.size()
    }

    /// Allocates a layout, returning the identifying offset.
    /// 
    /// The allocated object is identified by its offset value as real pointers could change at runtime
    /// because data can be swapped from volatile to non-volatile and the other time round at runtime.
    pub(crate) unsafe fn allocate<T, P: PageStorageModule>(&mut self, options: AllocationOptions<T>, page_storage: &mut P) -> Result<usize, AllocationOptions<T>> {
        let max_alloc_size = self.max_size_hint;

        self.dirty = true;

        // make sure that heap reference stays in ram
        self.mut_count.fetch_add(1, Ordering::SeqCst);

        // make sure the heap is in ram and get pointer to it
        let heap = self.get_heap(page_storage);
        let res = heap.allocate(&options.layout, max_alloc_size);
        let (res_ptr, res_size) = match res {
            Ok(res) => res,
            Err(_) => {
                // free mut reference
                self.mut_count.fetch_sub(1, Ordering::SeqCst);
                return Err(options);
            }
        };

        // write initial value
        (res_ptr.as_ptr() as *mut T).write(options.initial_value);

        let offset = ptr_to_offset(heap, res_ptr.as_ptr());

        // free mut reference
        self.mut_count.fetch_sub(1, Ordering::SeqCst);
        self.max_size_hint = res_size;

        Ok(offset as usize)
    }

    /// Deallocate an object from this heap.
    /// 
    /// The object is identified by its offset value as real pointers could change at runtime
    /// because data can be swapped from volatile to non-volatile and the other time round at runtime.
    pub(crate) unsafe fn dealloc<P: PageStorageModule>(&mut self, offset: usize, layout: &Layout, page_storage: &mut P) {
        let max_alloc_size = self.max_size_hint;
        
        self.dirty = true;

        // make sure that heap reference stays in ram
        self.mut_count.fetch_add(1, Ordering::SeqCst);

        // make sure heap is in ram and get pointer to it
        let heap = self.get_heap(page_storage);

        // get pointer to object from offset
        let ptr = offset_to_ptr(heap, offset);
        let result = heap.dealloc(NonNull::new(ptr).unwrap(), layout, max_alloc_size);

        self.mut_count.fetch_sub(1, Ordering::SeqCst);

        // update metadata
        self.max_size_hint = result;
    }

    /// Gets an immutable reference.
    /// The heap will be resident until this reference is dropped.
    /// 
    /// **Requirements** (or else heap implementation will break):
    /// - The data inside the `self_ref_cell` has to be equal to `self`
    /// - `offset` has to be valid:
    ///   - Should not be greater than the size of this heap
    ///   - Should point to valid and aligned data of type `T`
    pub(crate) unsafe fn get_ref<'b, T, P: PageStorageModule>(&mut self, offset: usize, page_storage: &mut P) -> *const T {
        self.ref_count.fetch_add(1, Ordering::SeqCst);

        let heap = self.get_heap(page_storage);
        let ptr = offset_to_ptr(heap, offset) as *const T;

        ptr
    }

    /// Gets a mutable reference.
    /// The heap will be resident until this reference is dropped.
    /// 
    /// **Requirements** (or else heap implementation will break):
    /// - The data inside the `self_ref_cell` has to be equal to `self`
    /// - `offset` has to be valid:
    ///   - Should not be greater than the size of this heap
    ///   - Should point to valid and aligned data of type `T`
    pub(crate) unsafe fn get_mut<'b, T, P: PageStorageModule>(&mut self, offset: usize, page_storage: &mut P) -> &'b mut T {
        self.mut_count.fetch_add(1, Ordering::SeqCst);

        let heap = self.get_heap(page_storage);
        let ptr = offset_to_ptr(heap, offset);
        let data_ref: &'b mut T = (ptr as *mut T).as_mut().unwrap();

        data_ref
    }

    /// Releases a reference
    /// 
    /// **NOTE**: Calling this function twice with the same reference will break this heap!
    pub(crate) unsafe fn release_ref<T, P: PageStorageModule>(&mut self, _: &T) {
        self.ref_count.fetch_sub(1, Ordering::SeqCst);
    }

    /// Releases a mutable reference
    /// 
    /// **NOTE**: Calling this function twice with the same reference will break this heap!
    pub(crate) unsafe fn release_mut<T, P: PageStorageModule>(&mut self, _: &mut T) {
        self.mut_count.fetch_sub(1, Ordering::SeqCst);
    }

    /// Makes sure that heap is in memory and returns the heap
    fn get_heap<P: PageStorageModule>(&mut self, page_storage: &mut P) -> &mut A {
        let res = unsafe {
            self.heap_ptr.as_mut()
        };

        if let Some(res) = res {
            // heap is already resident
            return res;
        }

        debug!("Mapping heap {self:p}");

        // not already in memory, load now...
        let map_res = unsafe { 
            page_storage.map(self.offset, self.size)
        }.unwrap();

        self.heap_ptr = map_res.as_ptr() as *mut A;
        unsafe {
            map_res.cast().as_mut()
        }
    }

    /// Unmaps this heap and syncs all pending changes back to the page storage
    pub(crate) fn unmap<P: PageStorageModule>(&mut self, page_storage: &mut P) {
        if self.heap_ptr.is_null() {
            // already unmapped
            return;
        }

        debug!("Unmapping heap {self:p}");

        // TODO: race conditions: should be fine...
        // but maybe think about it one more time

        if self.ref_count.load(Ordering::SeqCst) != 0 || self.mut_count.load(Ordering::SeqCst) != 0 {
            panic!("ehhm... cannot unmap because of open references...");
        }

        if self.dirty {
            self.persist(page_storage);
        }

        // TODO think about how to handle unwrap, maybe just ignore errors?
        unsafe {
            page_storage.unmap(NonNull::new(self.heap_ptr as *mut u8).unwrap(), self.size)
        }.unwrap();

        // TODO race condition here! If pfi occurs here, then this page is unmapped again...

        self.heap_ptr = null_mut();
    }

    /// Syncs all changes back to the page storage
    fn persist<P: PageStorageModule>(&mut self, page_storage: &mut P) {
        if self.heap_ptr.is_null() {
            // not mapped, nothing to sync
            return;
        }

        if !self.dirty {
            // not dirty, nothing to sync
            return;
        }

        debug!("Persisting heap {self:p}");

        // TODO think about how to handle unwrap, maybe just ignore errors?
        page_storage.persist(NonNull::new(self.heap_ptr as *mut u8).unwrap(), self.size).unwrap();

        // should be free from race conditions
        self.dirty = false;
    }

}

#[cfg(test)]
mod test {
    use std::sync::atomic::Ordering;

    use crate::{allocation_options::AllocationOptions, modules::{allocator::buddy::BuddyAllocatorModule, page_storage::{mmap::MMapPageStorageModule, PageStorageModule}}, vnv_heap_manager::{offset_to_ptr, ptr_to_offset, VNVHeapManager}};

    /// Tests allocation on a heap.
    /// Writes some data, saves it, unmaps it, maps it again and checks that it contains the required data. 
    #[test]
    fn test_alloc_sync() {
        const PAGE_SIZE: usize = 4096;

        let mut storage = MMapPageStorageModule::new("vnv_heap_meta_data_alloc_sync_test.tmp").unwrap();
        storage.add_new_region(PAGE_SIZE).unwrap();

        let mut meta: VNVHeapManager<BuddyAllocatorModule<16>> = VNVHeapManager::new(0, PAGE_SIZE, &mut storage);

        assert_eq!(meta.dirty, true);
        assert_eq!(meta.mut_count.load(Ordering::SeqCst), 0);
        assert_eq!(meta.ref_count.load(Ordering::SeqCst), 0);

        meta.persist(&mut storage);

        assert_eq!(meta.dirty, false);
        assert_eq!(meta.mut_count.load(Ordering::SeqCst), 0);
        assert_eq!(meta.ref_count.load(Ordering::SeqCst), 0);

        type TestSlice = [u8; 100];
        let offset = {
            let result = unsafe { meta.allocate(AllocationOptions::<TestSlice>::new([100u8; 100]), &mut storage) };
            let offset = match result {
                Ok(offset) => offset,
                Err(_) => panic!("could not allocate!")
            };

            let ptr = unsafe { offset_to_ptr(meta.heap_ptr.as_mut().unwrap(), offset) };

            assert_eq!(meta.dirty, true);
            assert_eq!(meta.mut_count.load(Ordering::SeqCst), 0);
            assert_eq!(meta.ref_count.load(Ordering::SeqCst), 0);
            assert!(!meta.heap_ptr.is_null());

            let mut_ref = unsafe { (ptr as *mut TestSlice).as_mut().unwrap() };

            // fill with some data... 
            for i in 0..mut_ref.len() {
                mut_ref[i] = (i * 2) as u8;
            }

            unsafe { ptr_to_offset(meta.get_heap(&mut storage), ptr) }
        };

        meta.unmap(&mut storage);

        assert_eq!(meta.dirty, false);
        assert_eq!(meta.mut_count.load(Ordering::SeqCst), 0);
        assert_eq!(meta.ref_count.load(Ordering::SeqCst), 0);
        assert!(meta.heap_ptr.is_null());

        // load heap back into ram
        let heap = meta.get_heap(&mut storage);
        {
            let ptr = unsafe {
                offset_to_ptr(heap, offset)
            };

            assert_eq!(meta.dirty, false);
            assert_eq!(meta.mut_count.load(Ordering::SeqCst), 0);
            assert_eq!(meta.ref_count.load(Ordering::SeqCst), 0);
            assert!(!meta.heap_ptr.is_null());

            let data_ref = unsafe { (ptr as *mut TestSlice).as_ref().unwrap() };

            // fill with some data... 
            for i in 0..data_ref.len() {
                assert_eq!(data_ref[i], (i * 2) as u8, "data does not match");
            }
        }
    }
}