use core::{
    alloc::Layout,
    marker::PhantomData,
    mem::{align_of, size_of, MaybeUninit},
    ptr::null_mut,
    slice,
};

use log::debug;

use crate::{
    allocation_options::AllocationOptions,
    modules::{
        allocator::AllocatorModule, memory_provider::MemoryProviderModule,
        page_storage::PageStorageModule,
    },
    util::{bit_array::BitArray, ceil_div, padding_needed_for},
    vnv_heap_metadata::VNVHeapMetadata,
    vnv_meta_store_item::{VNVHeapMetadataListItem, VNVMetaStoreItem},
    vnv_resident_heap::{calc_min_pages_of_new_heap, VNVResidentHeap},
    vnv_resident_heap_manager::{
        calc_max_resident_heap_count, VNVResidentHeapListItem, VNVResidentHeapManager,
        VNVResidentHeapManagerConfig,
    },
};

/// An object that is used to identify a specific allocation (on which heap and which offset on that heap?)
///
/// This Identifier is used for getting references and deallocating a `VNVObject`
pub(crate) struct AllocationIdentifier<T, A: AllocatorModule + 'static> {
    heap_ptr: *mut VNVHeapMetadata<A>,
    offset: usize,

    /// bind generic T to this struct, to prevent bugs
    _phantom_data: PhantomData<T>,
}

/// Helper function: Tries to allocate a specific memory layout on a heap
///
/// Fails if there is not enough space left on this heap.
unsafe fn try_allocate<T, A: AllocatorModule + 'static>(
    options: AllocationOptions<T>,
    heap: *mut VNVResidentHeap<A>,
    resident_heap_manager: &mut VNVResidentHeapManager<A>,
) -> Result<AllocationIdentifier<T, A>, AllocationOptions<T>> {
    let (offset, meta_ptr) = unsafe { resident_heap_manager.allocate(heap, options) }?;

    Ok(AllocationIdentifier {
        heap_ptr: meta_ptr,
        offset,
        _phantom_data: PhantomData,
    })
}

/// Requires that `heap` is resident.
/// Returns a pointer to a `VNVResidentHeap` which is guaranteed to be not null.
fn require_resident<A: AllocatorModule, S: PageStorageModule>(
    heap: &mut VNVHeapMetadata<A>,
    resident_heap_manager: &mut VNVResidentHeapManager<A>,
    page_storage: &mut S,
) -> *mut VNVResidentHeap<A> {
    if !heap.resident_ptr.is_null() {
        // already resident
        return heap.resident_ptr;
    }

    unsafe { resident_heap_manager.map_heap(heap, false, page_storage) }
}

/// Returns the slice where the `VNVHeapManager` objects are saved to.
/// It will fill the remaining space of the mmapped page (size is `mmap_size`).
///
/// This function will also calculate where to place the list so its aligned properly as
/// `base_ptr` is the pointer to beginning of the page and the first object on the page will be a `VNVMetaStoreItem`.
///
/// ### Safety
///
/// Make sure `base_ptr` is properly aligned and `mmap_size` is correct.
unsafe fn get_aligned_manager_slice<'a, A: AllocatorModule + 'static>(
    base_ptr: *mut u8,
    mmap_size: usize,
) -> &'a mut [VNVHeapMetadataListItem<A>] {
    let mut list_start = (base_ptr as usize) + size_of::<VNVMetaStoreItem<A>>();

    let item_alignment = align_of::<VNVHeapMetadataListItem<A>>();
    let item_size = size_of::<VNVHeapMetadataListItem<A>>();

    // align list_start to item_alignment
    list_start = (list_start + item_alignment - 1) & !(item_alignment - 1);

    // remaining space for the list (in bytes)
    let rem_size_for_list = mmap_size - (list_start - (base_ptr as usize));
    let item_count = rem_size_for_list / item_size;

    debug_assert_eq!(
        list_start % item_alignment,
        0,
        "list_start should be aligned correctly"
    );
    debug_assert!(
        (base_ptr as usize) + size_of::<VNVMetaStoreItem<A>>() <= list_start,
        "pointer to list start should start after the VNVMetaStore object"
    );
    debug_assert!(
        (list_start + item_count * size_of::<VNVHeapMetadataListItem<A>>())
            <= (base_ptr as usize + mmap_size),
        "end of array should be still inside the mapped page"
    );

    let list_start = list_start as *mut VNVHeapMetadataListItem<A>;
    slice::from_raw_parts_mut(list_start, item_count)
}

type VNVMetaStoreBlockStart<A> = (
    VNVResidentHeapManager<'static, A>,
    VNVMetaStoreItem<'static, A>,
);

#[derive(Debug)]
struct VNVMetaStoreBlockInfo {
    resident_heap_list_offset: usize,
    resident_heap_list_length: usize,
    recovery_list_offset: usize,
    recovery_list_length: usize,
    metadata_list_offset: usize,
    metadata_list_length: usize,
    total_size: usize,
}

impl VNVMetaStoreBlockInfo {
    /// Calculates a new memory layout to store following things (in that order):
    ///
    /// - `resident_heap_manager: VNVResidentHeapManager`
    /// - `meta_store_item: VNVMetaStoreItem`
    /// - `heap_list: [VNVResidentHeapListItem]` (as many as needed by `config`)
    /// - `recovery_list: [u8]` (`recover_list.len() == ceil_div(heap_list.len(), 8)`)
    /// - `meta_list`: `[VNVHeapMetadataListItem]` (this list is at least one item long,
    /// and fills the rest of the available space until `total_size` (almost) is a multiple of `block_size`)
    fn new<A: AllocatorModule + 'static, M: MemoryProviderModule>(
        max_resident_size: usize,
        block_size: usize,
    ) -> Self {
        // start of resident_heap_list for VNVResidentHeapManager
        // no alignment needed for first type
        let mut resident_heap_list_offset = size_of::<VNVMetaStoreBlockStart<A>>();
        // align resident heap list
        resident_heap_list_offset += padding_needed_for(
            resident_heap_list_offset,
            align_of::<VNVResidentHeapListItem<A>>(),
        );

        let resident_heap_list_length = calc_max_resident_heap_count(max_resident_size);

        // start of recovery_list for VNVResidentHeapManager
        // no alignment needed for [u8]
        let recovery_list_offset = resident_heap_list_offset
            + size_of::<VNVResidentHeapListItem<A>>() * resident_heap_list_length;

        let recovery_list_length = ceil_div(resident_heap_list_length, 8);

        // start of metadata list for VNVMetaStoreItem
        let mut metadata_list_offset =
            recovery_list_offset + size_of::<u8>() * recovery_list_length;
        metadata_list_offset += padding_needed_for(
            metadata_list_offset,
            align_of::<VNVHeapMetadataListItem<A>>(),
        );

        let mut metadata_list_length = 1;

        let total_used_size =
            metadata_list_offset + metadata_list_length * size_of::<VNVHeapMetadataListItem<A>>();

        // fill available space
        let total_size = ceil_div(total_used_size, block_size) * block_size;
        let remaining_space = total_size - total_used_size;

        metadata_list_length += remaining_space / size_of::<VNVHeapMetadataListItem<A>>();

        VNVMetaStoreBlockInfo {
            resident_heap_list_offset,
            resident_heap_list_length,
            recovery_list_offset,
            recovery_list_length,
            metadata_list_offset,
            metadata_list_length,
            total_size,
        }
    }
}

/// Manages a `VNVResidentHeapManager` and a list of `VNVMetaStoreItem`s that are both stored on mmapped pages
pub(crate) struct VNVMetaStore<
    A: AllocatorModule + 'static,
    S: PageStorageModule,
    M: MemoryProviderModule,
> {
    /// TODO fix this 'static reference
    /// first item in meta store list
    head: &'static mut VNVMetaStoreItem<'static, A>,

    /// TODO fix this 'static reference
    resident_heap_manager: &'static mut VNVResidentHeapManager<'static, A>,

    page_storage: S,

    /// cached block size
    block_size: usize,

    /// next offset that is free to be used
    curr_storage_offset: usize,

    /// used for cleaning up
    initial_max_resident_size: usize,

    _phantom_data: PhantomData<M>,
}

impl<A: AllocatorModule, S: PageStorageModule, M: MemoryProviderModule> VNVMetaStore<A, S, M> {
    pub(crate) fn new(page_storage: S, mut config: VNVResidentHeapManagerConfig) -> Self {
        let initial_max_resident_size = config.max_resident_size;

        // minimum amount of bytes for new meta store block
        let block_size = M::min_size();

        if config.max_resident_size <= block_size {
            panic!("config.max_resident_size too small!");
        }

        if config.max_dirty_size <= block_size {
            panic!("config.max_dirty_size too small!");
        }

        // get minimal layout,
        let minimal_layout =
            VNVMetaStoreBlockInfo::new::<A, M>(initial_max_resident_size - block_size, block_size);

        println!("{:?}", minimal_layout);

        if config.max_resident_size <= minimal_layout.total_size {
            panic!("config.max_resident_size too small for minimal layout!");
        }

        if config.max_dirty_size <= minimal_layout.total_size {
            panic!("config.max_dirty_size too small for minimal layout!");
        }

        config.max_resident_size -= minimal_layout.total_size;
        config.max_dirty_size -= minimal_layout.total_size;

        let base_ptr = unsafe { M::map_block(minimal_layout.total_size) };

        // ###############################
        // okay we got a new memory block
        // now we need to initialize all the data
        // ###############################

        // helper function
        unsafe fn init_list<'a, T>(
            base_ptr: *mut u8,
            offset: usize,
            length: usize,
            default_value: fn() -> T,
        ) -> &'a mut [T] {
            let base_item = unsafe { base_ptr.offset(offset as isize) } as *mut T;

            let mut curr_item = base_item;

            for _ in 0..length {
                unsafe { curr_item.write(default_value()) };

                // get next item
                curr_item = unsafe { curr_item.offset(1) };
            }

            unsafe { slice::from_raw_parts_mut(base_item, length) }
        }

        // ### VNVResidentHeapListItem<A> ###
        let resident_heap_list = unsafe {
            init_list::<VNVResidentHeapListItem<A>>(
                base_ptr,
                minimal_layout.resident_heap_list_offset,
                minimal_layout.resident_heap_list_length,
                || None,
            )
        };

        // ### recovery_list ###
        let recovery_list = unsafe {
            init_list::<u8>(
                base_ptr,
                minimal_layout.recovery_list_offset,
                minimal_layout.recovery_list_length,
                // recovery_list does not need to be initialized: its only ints and BitArray will set default values
                || 0,
            )
        };

        // ### VNVHeapMetadataListItem<A> ###
        let metadata_list = unsafe {
            init_list::<VNVHeapMetadataListItem<A>>(
                base_ptr,
                minimal_layout.metadata_list_offset,
                minimal_layout.metadata_list_length,
                || MaybeUninit::uninit(),
            )
        };

        // ### VNVResidentHeapManager and VNVMetaStoreItem ###
        let start_val = base_ptr as *mut VNVMetaStoreBlockStart<A>;
        let (resident_page_manager, meta_store_item) = unsafe {
            start_val.write((
                VNVResidentHeapManager::new(
                    resident_heap_list,
                    BitArray::new(recovery_list),
                    config,
                ),
                VNVMetaStoreItem::new(metadata_list),
            ));
            start_val.as_mut().unwrap()
        };

        VNVMetaStore {
            page_storage: page_storage,
            head: meta_store_item,
            resident_heap_manager: resident_page_manager,
            block_size: M::min_size(),
            curr_storage_offset: 0,
            _phantom_data: PhantomData,
            initial_max_resident_size,
        }
    }

    /// Returns the size of the meta data pages
    ///
    /// ### Safety
    ///
    /// - This should return the same value for the whole lifetime of `self`!
    /// - Return value is multiples of a page size
    ///
    /// If one of these points are violated, it will break `VNVMetaStore`s implementation.
    const fn get_mmap_size(&self) -> usize {
        self.block_size
    }

    /// Allocates `layout`. If no empty heap is found, it will create one
    ///
    /// ### Safety
    ///
    /// This function is unsafe as you have to make sure that this allocated
    /// memory will get deallocated again eventually
    pub(crate) unsafe fn allocate<T>(
        &mut self,
        options: AllocationOptions<T>,
    ) -> AllocationIdentifier<T, A> {
        let options = match self.allocate_on_fitting_heap(options) {
            Ok(res) => return res,
            Err(options) => options,
        };

        debug!(
            "Tried to allocate {} bytes but no fitting heap found. Creating new one...",
            options.layout.size()
        );

        // find empty meta store item
        let mut curr = self.head as *mut VNVMetaStoreItem<'static, A>;
        while !curr.is_null() {
            let curr_ref = unsafe { curr.as_mut().unwrap() };
            if curr_ref.get_element_count() < curr_ref.get_capacity() {
                let size_in_pages =
                    calc_min_pages_of_new_heap::<A>(self.block_size, &options.layout);
                let size = size_in_pages * self.block_size;
                let prev_offset = self.curr_storage_offset as u64;

                self.page_storage.add_new_region(size).unwrap();
                self.curr_storage_offset += size;

                let item = self.add_new_item();
                let heap = item.add_new_heap(prev_offset, size).unwrap();
                let resident_heap =
                    self.resident_heap_manager
                        .map_heap(heap, true, &mut self.page_storage);

                return try_allocate::<T, A>(
                    options,
                    resident_heap,
                    &mut self.resident_heap_manager,
                )
                .map_err(|_| ())
                .unwrap();
            }

            curr = curr_ref.next;
        }

        debug!("Could not find free slot in a VNVMetaStoreItem. Adding new one...");

        let size_in_pages = calc_min_pages_of_new_heap::<A>(self.block_size, &options.layout);
        let size = size_in_pages * self.block_size;
        let prev_offset = self.curr_storage_offset as u64;

        self.page_storage.add_new_region(size).unwrap();
        self.curr_storage_offset += size;

        let item = self.add_new_item();
        let heap = item.add_new_heap(prev_offset, size).unwrap();
        let resident_heap = self
            .resident_heap_manager
            .map_heap(heap, true, &mut self.page_storage);

        try_allocate::<T, A>(options, resident_heap, &mut self.resident_heap_manager)
            .map_err(|_| ())
            .unwrap()
    }

    pub(crate) unsafe fn deallocate<T>(
        &mut self,
        layout: &Layout,
        identifier: &AllocationIdentifier<T, A>,
    ) {
        // can safely get mutable reference as at
        // this moment no other part of code has mutable reference
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        let resident_heap =
            require_resident(heap_ref, self.resident_heap_manager, &mut self.page_storage);

        self.resident_heap_manager
            .deallocate::<T>(resident_heap, identifier.offset, layout);
    }

    pub(crate) unsafe fn get_mut<T>(&mut self, identifier: &AllocationIdentifier<T, A>) -> *mut T {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        let resident_heap =
            require_resident(heap_ref, self.resident_heap_manager, &mut self.page_storage);

        self.resident_heap_manager
            .get_mut(resident_heap, identifier.offset, &mut self.page_storage)
    }

    pub(crate) unsafe fn get_ref<T>(
        &mut self,
        identifier: &AllocationIdentifier<T, A>,
    ) -> *const T {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        let resident_heap =
            require_resident(heap_ref, self.resident_heap_manager, &mut self.page_storage);

        self.resident_heap_manager
            .get_ref(resident_heap, identifier.offset)
    }

    pub(crate) unsafe fn release_mut<T>(
        &mut self,
        identifier: &AllocationIdentifier<T, A>,
        data: &mut T,
    ) {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        let resident_heap =
            require_resident(heap_ref, self.resident_heap_manager, &mut self.page_storage);

        self.resident_heap_manager.release_mut(resident_heap, data);
    }

    pub(crate) unsafe fn release_ref<T>(
        &mut self,
        identifier: &AllocationIdentifier<T, A>,
        data: &T,
    ) {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        let resident_heap =
            require_resident(heap_ref, self.resident_heap_manager, &mut self.page_storage);

        self.resident_heap_manager.release_ref(resident_heap, data);
    }

    /// Searches for a fitting heap to allocate a new memory layout.
    ///
    /// Fails if all of the current heaps do not have enough memory left to allocate `layout`.
    ///
    /// ### Safety
    ///
    /// This function is unsafe as you have to make sure that this allocated memory will get deallocated again eventually
    unsafe fn allocate_on_fitting_heap<T>(
        &mut self,
        mut options: AllocationOptions<T>,
    ) -> Result<AllocationIdentifier<T, A>, AllocationOptions<T>> {
        // TODO: change this later
        let mut curr = self.head as *mut VNVMetaStoreItem<'static, A>;
        while !curr.is_null() {
            let curr_ref = unsafe { curr.as_mut().unwrap() };

            // iterate over all heap managers
            for i in 0..curr_ref.get_element_count() {
                let heap = curr_ref.get_item(i).unwrap();
                if heap.has_space_left(&options.layout) {
                    let resident_heap =
                        require_resident(heap, self.resident_heap_manager, &mut self.page_storage);

                    options = match try_allocate::<T, A>(
                        options,
                        resident_heap,
                        &mut self.resident_heap_manager,
                    ) {
                        // successfully allocated
                        Ok(res) => {
                            return Ok(res);
                        }
                        Err(options) => options,
                    };
                }
            }

            curr = curr_ref.next;
        }

        Err(options)
    }

    /// TODO fix this 'static reference
    /// Adds a new `VNVMetaStoreItem` to its list and allocates a new page for it
    fn add_new_item(&mut self) -> &'static mut VNVMetaStoreItem<'static, A> {
        let mmap_size = self.get_mmap_size();
        let base_ptr = unsafe { M::map_block(mmap_size) };

        unsafe {
            self.resident_heap_manager.decrease_resident_size(mmap_size);
            self.resident_heap_manager.decrease_dirty_size(mmap_size);
        };

        let manager_slice = unsafe { get_aligned_manager_slice::<A>(base_ptr, mmap_size) };

        // pointer to MetaStoreItem object at start of page
        let meta_ptr = base_ptr as *mut VNVMetaStoreItem<A>;

        debug!(
            "Adding new VNVMetaStoreItem: capacity={}",
            manager_slice.len()
        );
        unsafe {
            // write so uninitialized data does not get dropped
            meta_ptr.write(VNVMetaStoreItem::new(manager_slice));

            self.push_back(meta_ptr);
            meta_ptr.as_mut().unwrap()
        }
    }

    /// Pushes `VNVMetaStoreItem` to the back of the meta store list
    ///
    /// ### Safety
    ///
    /// Make sure `item` is a correct pointer
    unsafe fn push_back(&mut self, item: *mut VNVMetaStoreItem<'static, A>) {
        let mut curr = &mut self.head.next;

        while !(*curr).is_null() {
            curr = &mut (**curr).next;
        }

        *curr = item;
    }

    /// Pops an item from the front of the list.
    ///
    /// Returns `null_mut()` if the list only contains the head.
    fn pop_item(&mut self) -> *mut VNVMetaStoreItem<'static, A> {
        let ptr = self.head.next;
        if ptr.is_null() {
            return null_mut();
        }

        let curr = unsafe { ptr.as_mut().unwrap() };
        self.head.next = curr.next;
        curr.next = null_mut();
        ptr
    }
}

impl<A: AllocatorModule, S: PageStorageModule, M: MemoryProviderModule> Drop
    for VNVMetaStore<A, S, M>
{
    fn drop(&mut self) {
        // STEP 1: Unmap all pages in VNVMetaStoreItem list (except from `head` one of course)
        while !self.head.next.is_null() {
            let item = self.pop_item();

            // how many VNVHeapManager structs have been initialized?
            let initialized_items = unsafe { item.as_mut() }.unwrap().get_element_count();

            // drop the VNVMetaStoreItem first
            unsafe {
                item.drop_in_place();
            }

            let manager_slice =
                unsafe { get_aligned_manager_slice::<A>(item as *mut u8, self.get_mmap_size()) };
            // now drop all initialized `VNVHeapManager` items
            for i in 0..initialized_items {
                unsafe { manager_slice[i].assume_init_drop() }
            }

            // dropped all values, now unmap page
            unsafe {
                M::unmap_block(item as *mut u8, self.block_size);
            }
        }

        // STEP 2: Unmap MetaStoreBlock
        let info = VNVMetaStoreBlockInfo::new::<A, M>(
            self.initial_max_resident_size - self.block_size,
            self.block_size,
        );
        let base_ptr =
            (self.resident_heap_manager as *mut VNVResidentHeapManager<'static, A>) as *mut u8;

        // STEP 2.1: Prepare VNVResidentHeapManager for drop (aka unmap all resident pages)
        self.resident_heap_manager
            .before_drop(&mut self.page_storage);

        // STEP 2.2: Drop VNVResidentHeapManager & VNVMetaStoreItem
        unsafe {
            let resident_heap_manager_ptr =
                self.resident_heap_manager as *mut VNVResidentHeapManager<'static, A>;
            resident_heap_manager_ptr.drop_in_place();

            let store_item_ptr = self.head as *mut VNVMetaStoreItem<'static, A>;
            store_item_ptr.drop_in_place();
        }

        // STEP 2.3: Drop other lists

        // helper function to drop a list of type T
        unsafe fn drop_list<T>(base_ptr: *mut u8, offset: usize, length: usize) {
            let mut curr_item = unsafe { base_ptr.offset(offset as isize) } as *mut T;
            for i in 0..length {
                curr_item.drop_in_place();

                // get next item
                curr_item = unsafe { curr_item.offset(1) };
            }
        }

        unsafe {
            drop_list::<VNVResidentHeapListItem<A>>(
                base_ptr,
                info.resident_heap_list_offset,
                info.resident_heap_list_length,
            );
            drop_list::<u8>(
                base_ptr,
                info.recovery_list_offset,
                info.recovery_list_length,
            );
            drop_list::<VNVHeapMetadataListItem<A>>(
                base_ptr,
                info.metadata_list_offset,
                info.metadata_list_length,
            );
        }

        // STEP 2.4: Unmap MetaStoreBlock
        unsafe { M::unmap_block(base_ptr, info.total_size) };

        // finally complete...
    }
}

/*
#[cfg(test)]
pub(crate) mod test {
    use core::{array, ptr::null_mut};
    use crate::{modules::{allocator::{buddy::BuddyAllocatorModule, AllocatorModule}, page_storage::{mmap::MMapPageStorageModule, PageStorageModule}}, vnv_resident_heap::VNVResidentHeap, vnv_meta_store_item::VNVMetaStoreItem};

    use super::{get_aligned_manager_slice, AllocationIdentifier, VNVMetaStore};

    pub(crate) fn allocation_identifier_to_heap<T, A: AllocatorModule>(identifier: &AllocationIdentifier<T, A>) -> *mut VNVResidentHeap<A> {
        identifier.heap_ptr
    }

    /// check if `push_back` works as expected
    #[test]
    fn test_push_back() {
        let mut meta_store: VNVMetaStore<BuddyAllocatorModule<32>, MMapPageStorageModule> = VNVMetaStore::new();

        let mut objects: [VNVMetaStoreItem<'_, BuddyAllocatorModule<32>, MMapPageStorageModule>; 10] = array::from_fn(|_| VNVMetaStoreItem::new(&mut []));
        let list = [3usize, 9, 6, 2, 1, 7, 5, 0, 4, 8];

        assert!(meta_store.head.is_null());

        for i in 0..10 {
            let ptr = (&mut objects[list[i]]) as *mut VNVMetaStoreItem<'_, BuddyAllocatorModule<32>, MMapPageStorageModule>;

            // this is a bit unsafe as meta store will try to unmap these pointers later
            // so make sure you remove them before the meta store gets dropped (:
            unsafe { meta_store.push_back(ptr) }

            // pushed new item to back, now check if list is in desired state
            let mut curr = meta_store.head;
            for y in 0..=i {
                let expected = (&mut objects[list[y]]) as *mut VNVMetaStoreItem<'_, BuddyAllocatorModule<32>, MMapPageStorageModule>;
                assert_eq!(curr, expected);
                curr = (unsafe { &mut *curr }).next;
            }
            assert!(curr.is_null());
        }

        // meta store will try to unmap pages when it is dropped
        // so make sure to remove the pointer
        meta_store.head = null_mut();
        drop(meta_store);
    }

    #[test]
    fn test_add_item() {
        let mut meta_store: VNVMetaStore<BuddyAllocatorModule<32>, MMapPageStorageModule> = VNVMetaStore::new();
        assert!(meta_store.head.is_null());

        let item = meta_store.add_new_item();
        assert!(item.next.is_null());
        assert_eq!(item.get_element_count(), 0);

        let item = meta_store.add_new_item();
        assert_eq!(item.get_element_count(), 0);

        drop(meta_store);
    }

    #[test]
    fn test_aligned_manager_slice_independent() {
        let mut page_storage = MMapPageStorageModule::new("aligned_manager_slice_independent_test.tmp").unwrap();

        const PAGE_SIZE: usize = 4096;
        page_storage.add_new_region(PAGE_SIZE).unwrap();

        let mut virtual_page = [0u8; PAGE_SIZE];
        let slice = unsafe { get_aligned_manager_slice::<BuddyAllocatorModule<32>, MMapPageStorageModule>(&mut virtual_page as *mut u8, PAGE_SIZE) };
        slice[0].write(VNVResidentHeap::new(0, PAGE_SIZE, &mut page_storage));
    }

    #[test]
    fn test_aligned_manager_slice() {
        let mut meta_store: VNVMetaStore<BuddyAllocatorModule<32>, MMapPageStorageModule> = VNVMetaStore::new();
        let mut page_storage = MMapPageStorageModule::new("aligned_manager_slice_test.tmp").unwrap();

        const PAGE_SIZE: usize = 4096;
        page_storage.add_new_region(PAGE_SIZE).unwrap();

        let base = meta_store.add_new_item() as *mut VNVMetaStoreItem<'_, BuddyAllocatorModule<32>, MMapPageStorageModule>;

        let slice = unsafe { get_aligned_manager_slice::<BuddyAllocatorModule<32>, MMapPageStorageModule>(base as *mut u8, PAGE_SIZE) };
        slice[0].write(VNVResidentHeap::new(0, PAGE_SIZE, &mut page_storage));
    }
    #[test]
    fn test_aligned_manager_slice2() {
        let mut meta_store: VNVMetaStore<BuddyAllocatorModule<32>, MMapPageStorageModule> = VNVMetaStore::new();
        let mut page_storage = MMapPageStorageModule::new("aligned_manager_slice2_test.tmp").unwrap();

        const PAGE_SIZE: usize = 4096;
        page_storage.add_new_region(PAGE_SIZE).unwrap();

        let base = meta_store.add_new_item();
        base.add_new_heap(0, PAGE_SIZE, &mut page_storage).unwrap();
    }
}
*/
