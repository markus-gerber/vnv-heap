use std::{alloc::Layout, marker::PhantomData, mem::{align_of, size_of, MaybeUninit}, ptr::null_mut, slice};

use libc::{c_void, mmap, munmap, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use log::debug;

use crate::{allocation_options::AllocationOptions, modules::{allocator::AllocatorModule, page_storage::PageStorageModule}, util::get_page_size, vnv_heap_manager::{calc_min_pages_of_new_heap, VNVHeapManager}, vnv_meta_store_item::VNVMetaStoreItem, vnv_mut_ref::VNVMutRef};

/// An object that is used to identify a specific allocation (on which heap and which offset on that heap?)
/// 
/// This Identifier is used for getting references and deallocating a `VNVObject`
pub(crate) struct AllocationIdentifier<T, A: AllocatorModule> {
    heap_ptr: *mut VNVHeapManager<A>,
    offset: usize,

    /// bind generic T to this struct, to prevent bugs
    _phantom_data: PhantomData<T>
}

/// Tries to allocate a specific memory layout on a heap
/// 
/// Fails if there is not enough space left on this heap.
unsafe fn try_allocate<T, A: AllocatorModule, S: PageStorageModule>(options: AllocationOptions<T>, page_storage: &mut S, heap: &mut VNVHeapManager<A>) -> Result<AllocationIdentifier<T, A>, AllocationOptions<T>> {
    let offset = unsafe { heap.allocate(options, page_storage) }?;

    Ok(AllocationIdentifier {
        heap_ptr: heap as *mut VNVHeapManager<A>,
        offset,
        _phantom_data: PhantomData
    })
}

/// Returns the slice where the `VNVHeapManager` objects are saved to.
/// It will fill the remaining space of the mmapped page (size is `mmap_size`).
/// 
/// This function will also calculate where to place the list so its aligned properly as
/// `base_ptr` is the pointer to beginning of the page and the first object on the page will be a `VNVMetaStoreItem`.
/// 
/// **Safety**: Make sure `base_ptr` is properly aligned and `mmap_size` is correct. 
unsafe fn get_aligned_manager_slice<'a, A: AllocatorModule + 'static, S: PageStorageModule>(base_ptr: *mut u8, mmap_size: usize) -> &'a mut [MaybeUninit<VNVHeapManager<A>>] {
    let mut list_start = (base_ptr as usize) + size_of::<VNVMetaStoreItem<A, S>>();

    let item_alignment = align_of::<MaybeUninit<VNVHeapManager<A>>>();
    let item_size = size_of::<MaybeUninit<VNVHeapManager<A>>>();

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
        (base_ptr as usize) + size_of::<VNVMetaStoreItem<A, S>>() <= list_start,
        "pointer to list start should start after the VNVMetaStore object"
    );
    debug_assert!(
        (list_start + item_count * size_of::<MaybeUninit<VNVHeapManager<A>>>()) <= (base_ptr as usize + mmap_size),
        "end of array should be still inside the mapped page"
    );

    let list_start = list_start as *mut MaybeUninit<VNVHeapManager<A>>;
    slice::from_raw_parts_mut(list_start, item_count)
}


/// Manages a list of `VNVMetaStoreItem`s that are stored on mmapped pages (includes managing mapping and unmapping these pages)
pub(crate) struct VNVMetaStore<A: AllocatorModule + 'static, S: PageStorageModule> {
    /// first item in meta store list
    head: *mut VNVMetaStoreItem<'static, A, S>,

    /// cached page size
    page_size: usize,

    /// next offset that is free to be used
    curr_storage_offset: usize
}

impl<A: AllocatorModule, S: PageStorageModule> VNVMetaStore<A, S> {
    pub(crate) fn new() -> Self {
        VNVMetaStore {
            head: null_mut(),
            page_size: get_page_size(),
            curr_storage_offset: 0
        }
    }

    /// Returns the size of the meta data pages
    /// 
    /// ### SAFETY NOTES
    /// 
    /// - This should return the same value for the whole lifetime of `self`!
    /// - Return value is multiples of a page size
    /// 
    /// If one of these points are violated, it will break `VNVMetaStore`s implementation.
    const fn get_mmap_size(&self) -> usize {
        self.page_size
    }

    /// Allocates `layout`. If no empty heap is found, it will create one
    /// 
    /// **Safety**: This function is unsafe as you have to make sure that this allocated memory will get deallocated again eventually
    pub(crate) unsafe fn allocate<T>(&mut self, options: AllocationOptions<T>, page_storage: &mut S) -> AllocationIdentifier<T, A> {
        let options = match self.allocate_on_fitting_heap(options, page_storage) {
            Ok(res) => return res,
            Err(options) => options
        };

        debug!("Tried to allocate {} bytes but no fitting heap found. Creating new one...", options.layout.size());

        // find empty meta store item
        let mut curr = self.head;
        while !curr.is_null() {
            let curr_ref = unsafe { curr.as_mut().unwrap() };
            if curr_ref.get_element_count() < curr_ref.get_capacity() {
                let size_in_pages = calc_min_pages_of_new_heap::<A>(self.page_size, &options.layout);
                let size = size_in_pages * self.page_size;        
                let prev_offset = self.curr_storage_offset as u64;

                page_storage.add_new_region(size).unwrap();
                self.curr_storage_offset += size;
                
                let item = self.add_new_item();
                let heap = item.add_new_heap(prev_offset, size, page_storage).unwrap();

                return try_allocate(options, page_storage, heap).map_err(|_| ()).unwrap();
            }

            curr = curr_ref.next;
        }

        debug!("Could not find free slot in a VNVMetaStoreItem. Adding new one...");

        let size_in_pages = calc_min_pages_of_new_heap::<A>(self.page_size, &options.layout);
        let size = size_in_pages * self.page_size;        
        let prev_offset = self.curr_storage_offset as u64;

        page_storage.add_new_region(size).unwrap();
        self.curr_storage_offset += size;
        
        let item = self.add_new_item();
        let heap = item.add_new_heap(prev_offset, size, page_storage).unwrap();

        try_allocate(options, page_storage, heap).map_err(|_| ()).unwrap()
    }


    pub(crate) unsafe fn deallocate<T>(&mut self, layout: &Layout, page_storage: &mut S, identifier: &AllocationIdentifier<T, A>) {
        // can safely get mutable reference as at
        // this moment no other part of code has mutable reference
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        heap_ref.dealloc(identifier.offset, layout, page_storage);
    }

    pub(crate) unsafe fn get_mut<T>(&mut self, identifier: &AllocationIdentifier<T, A>, page_storage: &mut S) -> *mut T {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        heap_ref.get_mut(identifier.offset, page_storage)
    }

    pub(crate) unsafe fn get_ref<T>(&mut self, identifier: &AllocationIdentifier<T, A>, page_storage: &mut S) -> *const T {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        heap_ref.get_ref(identifier.offset, page_storage)        
    }


    pub(crate) unsafe fn release_mut<T>(&mut self, identifier: &AllocationIdentifier<T, A>, data: &mut T) {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        heap_ref.release_mut::<T, S>(data);
    }

    pub(crate) unsafe fn release_ref<T>(&mut self, identifier: &AllocationIdentifier<T, A>, data: &T) {
        let heap_ref = identifier.heap_ptr.as_mut().unwrap();
        heap_ref.release_ref::<T, S>(data);    
    }

    /// Searches for a fitting heap to allocate a new memory layout.
    /// 
    /// Fails if all of the current heaps do not have enough memory left to allocate `layout`.
    /// 
    /// **Safety**: This function is unsafe as you have to make sure that this allocated memory will get deallocated again eventually
    unsafe fn allocate_on_fitting_heap<T>(&mut self, mut options: AllocationOptions<T>, page_storage: &mut S) -> Result<AllocationIdentifier<T, A>, AllocationOptions<T>> {
        // TODO: change this later
        let mut curr = self.head;
        while !curr.is_null() {
            let curr_ref = unsafe { curr.as_mut().unwrap() };

            // iterate over all heap managers
            for i in 0..curr_ref.get_element_count() {
                let heap = curr_ref.get_item(i).unwrap();
                if heap.has_space_left(&options.layout) {
                    options = match try_allocate(options, page_storage, heap) {
                        // successfully allocated
                        Ok(res) => {return Ok(res);},
                        Err(options) => options
                    };
                }
            }

            curr = curr_ref.next;
        }

        Err(options)
    }

    /// Adds a new `VNVMetaStoreItem` to its list and allocates a new page for it
    fn add_new_item(&mut self) -> &mut VNVMetaStoreItem<'static, A, S> {
        let mmap_size = self.get_mmap_size();
        let base_ptr = unsafe {
            mmap(null_mut(), mmap_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0)
        };
        
        if base_ptr == MAP_FAILED {
            panic!("map failed");
        }

        let base_ptr = base_ptr as *mut u8;
        let manager_slice = unsafe { get_aligned_manager_slice::<A, S>(base_ptr, mmap_size) };

        // pointer to MetaStoreItem object at start of page
        let meta_ptr = base_ptr as *mut VNVMetaStoreItem<A, S>;

        debug!("Adding new VNVMetaStoreItem: capacity={}", manager_slice.len());
        unsafe {
            // write so uninitialized data does not get dropped
            meta_ptr.write(VNVMetaStoreItem::new(manager_slice));

            self.push_back(meta_ptr);
            meta_ptr.as_mut().unwrap()
        }
    }

    /// Pushes `VNVMetaStoreItem` to the back of the meta store list
    /// 
    /// **Note**: Make sure `item` is a correct pointer
    unsafe fn push_back(&mut self, item: *mut VNVMetaStoreItem<'static, A, S>) {
        let mut curr = &mut self.head;

        while !(*curr).is_null() {
            curr = &mut (**curr).next;
        }

        *curr = item;
    }

    /// Pops an item from the front of the list.
    /// 
    /// Returns `null_mut()` if the list is empty.
    fn pop_item(&mut self) -> *mut VNVMetaStoreItem<'static, A, S> {
        let ptr = self.head;
        if ptr.is_null() {
            return null_mut();
        }

        let curr = unsafe { ptr.as_mut().unwrap() };
        self.head = curr.next;
        curr.next = null_mut();
        ptr
    }
}

impl<A: AllocatorModule, S: PageStorageModule> Drop for VNVMetaStore<A, S>  {
    fn drop(&mut self) {
        // unmap all pages in VNVMetaStoreItem list
        while !self.head.is_null() {
            let item = self.pop_item();

            // how many VNVHeapManager structs have been initialized?
            let initialized_items = unsafe { item.as_mut() }.unwrap().get_element_count();

            // drop the VNVMetaStoreItem first
            unsafe {
                item.drop_in_place();
            }

            let manager_slice = unsafe { get_aligned_manager_slice::<A, S>(item as *mut u8, self.get_mmap_size()) };
            // now drop all initialized `VNVHeapManager` items
            for i in 0..initialized_items {
                unsafe { manager_slice[i].assume_init_drop() }
            }

            // dropped all values, now unmap page
            let code = unsafe {
                munmap(item as *mut c_void, self.get_mmap_size())
            };

            if code != 0 {
                println!("ERROR: Could not unmap meta page");
            }
        }

    }
}


#[cfg(test)]
pub(crate) mod test {
    use std::{array, ptr::null_mut};
    use crate::{modules::{allocator::{buddy::BuddyAllocatorModule, AllocatorModule}, page_storage::{mmap::MMapPageStorageModule, PageStorageModule}}, vnv_heap_manager::VNVHeapManager, vnv_meta_store_item::VNVMetaStoreItem};

    use super::{get_aligned_manager_slice, AllocationIdentifier, VNVMetaStore};

    pub(crate) fn allocation_identifier_to_heap<T, A: AllocatorModule>(identifier: &AllocationIdentifier<T, A>) -> &mut VNVHeapManager<A> {
        unsafe { identifier.heap_ptr.as_mut() }.unwrap()
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
        slice[0].write(VNVHeapManager::new(0, PAGE_SIZE, &mut page_storage));
    }

    #[test]
    fn test_aligned_manager_slice() {
        let mut meta_store: VNVMetaStore<BuddyAllocatorModule<32>, MMapPageStorageModule> = VNVMetaStore::new();
        let mut page_storage = MMapPageStorageModule::new("aligned_manager_slice_test.tmp").unwrap();

        const PAGE_SIZE: usize = 4096;
        page_storage.add_new_region(PAGE_SIZE).unwrap();

        let base = meta_store.add_new_item() as *mut VNVMetaStoreItem<'_, BuddyAllocatorModule<32>, MMapPageStorageModule>;

        let slice = unsafe { get_aligned_manager_slice::<BuddyAllocatorModule<32>, MMapPageStorageModule>(base as *mut u8, PAGE_SIZE) };
        slice[0].write(VNVHeapManager::new(0, PAGE_SIZE, &mut page_storage));
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