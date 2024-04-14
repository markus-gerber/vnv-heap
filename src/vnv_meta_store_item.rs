use std::{mem::MaybeUninit, ptr::null_mut};

use log::debug;

use crate::{modules::allocator::AllocatorModule, vnv_heap_metadata::VNVHeapMetadata};

// expose type of list to calculate storage location and offsets
pub(crate) type VNVHeapMetadataListItem<A> = MaybeUninit<VNVHeapMetadata<A>>;

/// An item inside the `VNVMetaStore` list.
/// Manages a list of `VNVHeapMetadata` objects.
pub(crate) struct VNVMetaStoreItem<'a, A: AllocatorModule + 'static> {
    /// Next item in the list. Managed by `VNVMetaStore`.
    pub(crate) next: *mut VNVMetaStoreItem<'a, A>,

    /// Amount of initialized elements of `data` list
    element_count: usize,

    /// Reference to the list of partly initialized `VNVHeapMetadata`s.
    /// (exactly `element_count` elements are initialized)
    ///
    /// *Not exactly the most efficient method, but the most safe and ergonomic in Rust (optimization could be done)*.
    data: &'a mut [VNVHeapMetadataListItem<A>],
}

impl<'a, A: AllocatorModule> VNVMetaStoreItem<'a, A> {
    /// Creates a new instance of VNVHeapMetaList with uninitialized list of `VNVHeapMetadata`s.
    pub(crate) fn new(list: &'a mut [VNVHeapMetadataListItem<A>]) -> Self {
        VNVMetaStoreItem {
            next: null_mut(),
            element_count: 0,
            data: list,
        }
    }

    pub(crate) fn get_capacity(&self) -> usize {
        self.data.len()
    }

    /// Gets the amount of initialized `VNVHeapMetadata`s.
    pub(crate) fn get_element_count(&self) -> usize {
        self.element_count
    }

    /// Gets an item, fails if `index >= self.element_count`.
    pub(crate) fn get_item(&mut self, index: usize) -> Option<&mut VNVHeapMetadata<A>> {
        if index >= self.element_count {
            return None;
        }

        Some(unsafe { self.data[index].assume_init_mut() })
    }

    /// Adds new heap to this `VNVMetaStoreItem`. Fails if no more space is left in this item.
    pub(crate) fn add_new_heap(
        &mut self,
        offset: u64,
        size: usize,
    ) -> Result<&mut VNVHeapMetadata<A>, ()> {
        if self.element_count >= self.data.len() {
            return Err(());
        }

        debug!(
            "Creating new Heap Metadata {}: offset={}, size={}",
            self.element_count, offset, size
        );

        let element = &mut self.data[self.element_count];
        // call write: don't drop uninitialized data
        let res = element.write(VNVHeapMetadata {
            size,
            offset,
            resident_ptr: null_mut(),
            max_size_hint: 0,
        });

        self.element_count += 1;

        Ok(res)
    }
}

#[cfg(test)]
mod test {
    use super::VNVMetaStoreItem;
    use crate::{
        modules::allocator::buddy::BuddyAllocatorModule, vnv_heap_metadata::VNVHeapMetadata,
    };
    use std::{array, mem::MaybeUninit};

    /// tests if capacity is never exceeded and get works as expected
    #[test]
    fn test_capacity_simple() {
        let mut arr: [MaybeUninit<VNVHeapMetadata<BuddyAllocatorModule<16>>>; 2] =
            array::from_fn(|_| MaybeUninit::uninit());
        let mut item: VNVMetaStoreItem<BuddyAllocatorModule<16>> = VNVMetaStoreItem::new(&mut arr);

        assert_eq!(item.data.len(), 2);
        assert_eq!(item.element_count, 0);
        test_get(&mut item, 0, 2);

        let res = item.add_new_heap(0, 4096);
        assert!(res.is_ok());

        assert_eq!(item.element_count, 1);
        test_get(&mut item, 1, 2);

        let res = item.add_new_heap(4096, 4096);
        assert!(res.is_ok());

        assert_eq!(item.element_count, 2);
        test_get(&mut item, 2, 2);

        let res = item.add_new_heap(8192, 4096);
        assert!(res.is_err(), "capacity exceeded");

        assert_eq!(item.element_count, 2);
        test_get(&mut item, 2, 2);
    }

    fn test_get(
        item: &mut VNVMetaStoreItem<BuddyAllocatorModule<16>>,
        init_cnt: usize,
        capacity: usize,
    ) {
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
