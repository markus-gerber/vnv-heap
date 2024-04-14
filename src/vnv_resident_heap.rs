use std::{
    alloc::Layout,
    mem::size_of,
    ptr::NonNull,
    sync::atomic::{AtomicU16, Ordering},
};

use crate::{
    allocation_options::AllocationOptions, modules::allocator::AllocatorModule,
    vnv_heap_metadata::VNVHeapMetadata,
};

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

    offset as usize
}

pub(crate) fn aligned_alloc_module_offset<A: AllocatorModule>() -> usize {
    let layout = Layout::new::<A>();

    let aligned_layout = layout.align_to(size_of::<usize>()).unwrap();
    let padded_layout = aligned_layout.pad_to_align();

    padded_layout.size()
}

/// Calculates the minimum size of a new heap in pages containing `layout` as multiples of `page_size`.
///
/// Includes that fact that the allocator module `A` is stored at the start of the new pages.
pub(crate) fn calc_min_pages_of_new_heap<A: AllocatorModule>(
    page_size: usize,
    layout: &Layout,
) -> usize {
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
pub(crate) struct VNVResidentHeap<A: AllocatorModule + Sized + 'static> {
    /// count of immutable references to this heap
    ref_count: AtomicU16,

    /// count of mutable references to this heap
    mut_count: AtomicU16,

    /// is this heap dirty? (are there unsynced changes)
    dirty: bool,

    /// pointer to resident allocator module
    heap_ref: &'static mut A,

    /// pointer to heaps metadata, is not null
    metadata: *mut VNVHeapMetadata<A>,
}

impl<A: AllocatorModule> VNVResidentHeap<A> {
    /// initializes the heap metadata
    pub(crate) fn new(heap_ref: &'static mut A, metadata: *mut VNVHeapMetadata<A>) -> Self {
        Self {
            ref_count: 0.into(),
            mut_count: 0.into(),
            dirty: true,
            heap_ref,
            metadata,
        }
    }

    pub(crate) fn get_metadata_ptr(&self) -> *mut VNVHeapMetadata<A> {
        self.metadata
    }

    pub(crate) fn get_heap_ref(&mut self) -> &mut A {
        self.heap_ref
    }

    pub(crate) fn get_ref_count(&self) -> u16 {
        self.ref_count.load(Ordering::SeqCst)
    }

    pub(crate) fn get_mut_count(&self) -> u16 {
        self.mut_count.load(Ordering::SeqCst)
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Allocates a layout, returning the identifying offset.
    ///
    /// The allocated object is identified by its offset value as real pointers could change at runtime
    /// because data can be swapped from volatile to non-volatile and the other time round at runtime.
    pub(crate) unsafe fn allocate<T>(
        &mut self,
        options: AllocationOptions<T>,
    ) -> Result<usize, AllocationOptions<T>> {
        let metadata_ref = self.metadata.as_mut().unwrap();
        let max_alloc_size = metadata_ref.max_size_hint;

        self.dirty = true;

        // make sure that heap reference stays in ram
        // TODO: ORDERING
        self.mut_count.fetch_add(1, Ordering::SeqCst);

        // make sure the heap is in ram and get pointer to it
        let res = self.heap_ref.allocate(&options.layout, max_alloc_size);
        let (res_ptr, res_size) = match res {
            Ok(res) => res,
            Err(_) => {
                // free mut reference
                // TODO: ORDERING
                self.mut_count.fetch_sub(1, Ordering::SeqCst);
                return Err(options);
            }
        };

        // write initial value
        (res_ptr.as_ptr() as *mut T).write(options.initial_value);

        let offset = ptr_to_offset(self.heap_ref, res_ptr.as_ptr());

        // free mut reference
        self.mut_count.fetch_sub(1, Ordering::SeqCst);
        metadata_ref.max_size_hint = res_size;

        Ok(offset as usize)
    }

    /// Deallocate an object from this heap.
    ///
    /// The object is identified by its offset value as real pointers could change at runtime
    /// because data can be swapped from volatile to non-volatile and the other time round at runtime.
    pub(crate) unsafe fn deallocate(&mut self, offset: usize, layout: &Layout) {
        let metadata_ref = self.metadata.as_mut().unwrap();
        let max_alloc_size = metadata_ref.max_size_hint;

        self.dirty = true;

        // make sure that heap reference stays in ram
        // TODO: ORDERING
        self.mut_count.fetch_add(1, Ordering::SeqCst);

        // get pointer to object from offset
        let ptr = offset_to_ptr(self.heap_ref, offset);
        let result = self
            .heap_ref
            .deallocate(NonNull::new(ptr).unwrap(), layout, max_alloc_size);

        // TODO: ORDERING
        self.mut_count.fetch_sub(1, Ordering::SeqCst);

        // update metadata
        metadata_ref.max_size_hint = result;
    }

    /// Gets an immutable reference.
    /// It will be ensure that this heap will be resident until this reference is dropped.
    ///
    /// ### Safety
    ///
    /// `offset` has to be valid:
    /// - Should not be greater than the size of this heap
    /// - Should point to valid and aligned data of type `T`
    pub(crate) unsafe fn get_ref<'b, T>(&mut self, offset: usize) -> &'b T {
        self.ref_count.fetch_add(1, Ordering::SeqCst);

        let ptr = offset_to_ptr(self.heap_ref, offset) as *const T;
        let data_ref: &'b T = ptr.as_ref().unwrap();

        data_ref
    }

    /// Gets a mutable reference.
    /// The heap will be resident until this reference is dropped.
    ///
    /// ### Safety
    ///
    /// `offset` has to be valid:
    /// - Should not be greater than the size of this heap
    /// - Should point to valid and aligned data of type `T`
    pub(crate) unsafe fn get_mut<'b, T>(&mut self, offset: usize) -> &'b mut T {
        self.mut_count.fetch_add(1, Ordering::SeqCst);

        let ptr = offset_to_ptr(self.heap_ref, offset) as *mut T;
        let data_ref: &'b mut T = ptr.as_mut().unwrap();

        data_ref
    }

    /// Releases a reference
    ///
    /// ### Safety
    ///
    /// Calling this function twice with the same reference will break this heap!
    pub(crate) unsafe fn release_ref<T>(&mut self, _: &T) {
        self.ref_count.fetch_sub(1, Ordering::SeqCst);
    }

    /// Releases a mutable reference
    ///
    /// ### Safety
    ///
    /// Calling this function twice with the same reference will break this heap!
    pub(crate) unsafe fn release_mut<T>(&mut self, _: &mut T) {
        self.mut_count.fetch_sub(1, Ordering::SeqCst);
    }

    pub(crate) fn has_mutable_references(&self) -> bool {
        self.mut_count.load(Ordering::SeqCst) != 0
    }

    pub(crate) fn has_immutable_references(&self) -> bool {
        self.ref_count.load(Ordering::SeqCst) != 0
    }
}

/*
#[cfg(test)]
mod test {
    use std::{mem::size_of, sync::atomic::Ordering};

    use crate::{allocation_options::AllocationOptions, modules::{allocator::{buddy::BuddyAllocatorModule, AllocatorModule}, page_storage::{mmap::MMapPageStorageModule, PageStorageModule}}, util::{ceil_div, get_page_size}, vnv_resident_heap::{offset_to_ptr, ptr_to_offset, VNVResidentHeap}};

    use super::aligned_alloc_module_offset;

    #[test]
    fn test_aligned_alloc_module_offset() {
        fn test_type<T: AllocatorModule>() {
            let ceiled_size = ceil_div(size_of::<T>(), size_of::<usize>()) * size_of::<usize>();
            assert_eq!(aligned_alloc_module_offset::<T>(), ceiled_size);
        }

        struct Test1 {
            _x: u8
        }
        struct Test2 {
            _x: usize
        }
        struct Test3 {
            _x: usize,
            _y: u8
        }

        macro_rules! impl_allocator_module {
            ($t:ident) => {
                impl AllocatorModule for $t {
                fn new() -> Self { panic!("dummy implementation") }
                    unsafe fn init(&mut self, _start: *mut u8, _size: usize) -> usize { panic!("dummy implementation") }
                    unsafe fn allocate(&mut self, _layout: &std::alloc::Layout, _max_alloc_size: usize) -> Result<(std::ptr::NonNull<u8>, usize), ()> { panic!("dummy implementation") }
                    unsafe fn deallocate(&mut self, _ptr: std::ptr::NonNull<u8>, _layout: &std::alloc::Layout, _max_alloc_size: usize) -> usize { panic!("dummy implementation") }
                    unsafe fn on_ptr_change(&mut self, _old_base_ptr: *mut u8, _new_base_ptr: *mut u8) { panic!("dummy implementation") }
                    fn calc_min_size_for_layout(_layout: &std::alloc::Layout) -> usize { panic!("dummy implementation") }
                }
            }
        }

        impl_allocator_module!(Test1);
        impl_allocator_module!(Test2);
        impl_allocator_module!(Test3);

        assert_eq!(aligned_alloc_module_offset::<Test1>(), size_of::<usize>());
        test_type::<Test1>();

        assert_eq!(aligned_alloc_module_offset::<Test2>(), size_of::<usize>());
        test_type::<Test2>();

        assert_eq!(aligned_alloc_module_offset::<Test3>(), size_of::<usize>() * 2);
        test_type::<Test3>();
    }

    /// Tests allocation on a heap.
    /// Writes some data, saves it, unmaps it, maps it again and checks that it contains the required data.
    #[test]
    fn test_alloc_sync() {
        const PAGE_SIZE: usize = get_page_size();

        let mut storage = MMapPageStorageModule::new("vnv_heap_meta_data_alloc_sync_test.tmp").unwrap();

        let ptr = unsafe { storage.map_new_region(PAGE_SIZE) }.unwrap().as_ptr();

        let mut heap: VNVResidentHeap<BuddyAllocatorModule<16>> = VNVResidentHeap::new(0, PAGE_SIZE);

        assert_eq!(heap.dirty, true);
        assert_eq!(heap.mut_count.load(Ordering::SeqCst), 0);
        assert_eq!(heap.ref_count.load(Ordering::SeqCst), 0);

        heap.persist(&mut storage);

        assert_eq!(heap.dirty, false);
        assert_eq!(heap.mut_count.load(Ordering::SeqCst), 0);
        assert_eq!(heap.ref_count.load(Ordering::SeqCst), 0);

        type TestSlice = [u8; 100];
        let offset = {
            let result = unsafe { heap.allocate(AllocationOptions::<TestSlice>::new([100u8; 100])) };
            let offset = match result {
                Ok(offset) => offset,
                Err(_) => panic!("could not allocate!")
            };

            let ptr = unsafe { offset_to_ptr(heap.heap_ref.as_mut().unwrap(), offset) };

            assert_eq!(heap.dirty, true);
            assert_eq!(heap.mut_count.load(Ordering::SeqCst), 0);
            assert_eq!(heap.ref_count.load(Ordering::SeqCst), 0);
            assert!(!heap.heap_ref.is_null());

            let mut_ref = unsafe { (ptr as *mut TestSlice).as_mut().unwrap() };

            // fill with some data...
            for i in 0..mut_ref.len() {
                mut_ref[i] = (i * 2) as u8;
            }

            unsafe { ptr_to_offset(heap.get_heap(&mut storage), ptr) }
        };

        heap.unmap(&mut storage);

        assert_eq!(heap.dirty, false);
        assert_eq!(heap.mut_count.load(Ordering::SeqCst), 0);
        assert_eq!(heap.ref_count.load(Ordering::SeqCst), 0);
        assert!(heap.heap_ref.is_null());

        // load heap back into ram
        let heap = heap.get_heap(&mut storage);
        {
            let ptr = unsafe {
                offset_to_ptr(heap, offset)
            };

            assert_eq!(heap.dirty, false);
            assert_eq!(heap.mut_count.load(Ordering::SeqCst), 0);
            assert_eq!(heap.ref_count.load(Ordering::SeqCst), 0);
            assert!(!heap.heap_ref.is_null());

            let data_ref = unsafe { (ptr as *mut TestSlice).as_ref().unwrap() };

            // fill with some data...
            for i in 0..data_ref.len() {
                assert_eq!(data_ref[i], (i * 2) as u8, "data does not match");
            }
        }
    }
}
*/
