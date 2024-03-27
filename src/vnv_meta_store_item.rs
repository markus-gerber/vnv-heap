use std::{mem::MaybeUninit, ptr::null_mut};

use crate::{modules::{allocator::AllocatorModule, page_storage::PageStorageModule}, vnv_heap_manager::VNVHeapManager};

/// An item inside the `VNVMetaStore` list.
/// Manages a list of `VNVHeapManager` objects.
pub(crate) struct VNVMetaStoreItem<'a, A: AllocatorModule, S: PageStorageModule> {
    /// Next item in the list. Managed by `VNVMetaStore`.
    pub(crate) next: *mut VNVMetaStoreItem<'a, A, S>,

    /// Amount of initialized elements of `data` list
    element_count: usize,

    /// Reference to the list of partly initialized `VNVHeapManager`s.
    /// (exactly `element_count` elements are initialized)
    /// 
    /// *Not exactly the most efficient method, but the most safe and ergonomic in Rust*.
    data: &'a mut [MaybeUninit<VNVHeapManager<A>>]
}

impl<'a, A: AllocatorModule, S: PageStorageModule> VNVMetaStoreItem<'a, A, S> {
    /// Creates a new instance of VNVHeapMetaList with uninitialized list of `VNVHeapManager`s.
    pub(crate) fn new(list: &'a mut [MaybeUninit<VNVHeapManager<A>>]) -> Self {
        VNVMetaStoreItem {
            next: null_mut(),
            element_count: 0,
            data: list
        }
    }

    pub(crate) fn get_capacity(&self) -> usize {
        self.data.len()
    }

    /// Gets the amount of initialized `VNVHeapManager`s.
    pub(crate) fn get_element_count(&self) -> usize {
        self.element_count
    }

    /// Gets an item, fails if `index >= self.element_count`.
    pub(crate) fn get_item(&mut self, index: usize) -> Option<&mut VNVHeapManager<A>> {
        if index >= self.element_count {
            return None;
        }

        Some(unsafe { self.data[index].assume_init_mut() })
    }

    /// Adds new heap to this `MetaStoreItem`. Fails if no more space is left in this item.
    pub(crate) fn add_new_heap(&mut self, offset: u64, size: usize, page_storage: &mut S) -> Result<&mut VNVHeapManager<A>, ()> {
        if self.element_count >= self.data.len() {
            return Err(());
        }

        let x= &mut self.data[self.element_count];
        // call write: don't drop uninitialized data
        let res = x.write(VNVHeapManager::new(offset, size, page_storage));
        self.element_count += 1;

        Ok(res)
    }

}

#[cfg(test)]
mod test {
    use std::{array, mem::MaybeUninit};
    use crate::{modules::{allocator::buddy::BuddyAllocatorModule, page_storage::{mmap::MMapPageStorageModule, PageStorageModule}}, vnv_heap_manager::VNVHeapManager};
    use super::VNVMetaStoreItem;

    /// tests if capacity is never exceeded and get works as expected
    #[test]
    fn test_capacity_simple() {
        let mut storage = MMapPageStorageModule::new("heap_meta_list_cap_test_smpl.tmp").unwrap();
        storage.add_new_region(4096 * 2).unwrap();

        let mut arr: [MaybeUninit<VNVHeapManager<BuddyAllocatorModule<16>>>; 2] = array::from_fn(|_| MaybeUninit::uninit());
        let mut item: VNVMetaStoreItem<BuddyAllocatorModule<16>, MMapPageStorageModule> = VNVMetaStoreItem::new(&mut arr);

        assert_eq!(item.data.len(), 2);
        assert_eq!(item.element_count, 0);
        test_get(&mut item, 0, 2);

        let res = item.add_new_heap(0, 4096, &mut storage);
        assert!(res.is_ok());
        
        assert_eq!(item.element_count, 1);
        test_get(&mut item, 1, 2);

        let res = item.add_new_heap(4096, 4096, &mut storage);
        assert!(res.is_ok());
        
        assert_eq!(item.element_count, 2);
        test_get(&mut item, 2, 2);

        let res = item.add_new_heap(8192, 4096, &mut storage);
        assert!(res.is_err(), "capacity exceeded");

        assert_eq!(item.element_count, 2);    
        test_get(&mut item, 2, 2);
    }

    fn test_get(item: &mut VNVMetaStoreItem<BuddyAllocatorModule<16>, MMapPageStorageModule>, init_cnt: usize, capacity: usize) {
        // (capacity + 2) to test some extra accesses
        for i in 0..(capacity + 2) {
            if i < init_cnt {
                assert!(item.get_item(i).is_some());
            } else {
                assert!(item.get_item(i).is_none());
            }
        }
    }

}