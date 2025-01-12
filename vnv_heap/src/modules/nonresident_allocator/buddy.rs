// code modified from: https://github.com/rcore-os/buddy_system_allocator

use log::trace;

use super::{NonResidentAllocatorModule, SimpleNonResidentLinkedList};
use crate::modules::persistent_storage::PersistentStorageModule;
use core::alloc::Layout;
use core::array;
use core::{
    cmp::{max, min},
    mem::size_of,
};

pub struct NonResidentBuddyAllocatorModule<const ORDER: usize> {
    /// buddy system with max order of `ORDER`
    free_list: [SimpleNonResidentLinkedList; ORDER],
}

impl<const ORDER: usize> NonResidentAllocatorModule for NonResidentBuddyAllocatorModule<ORDER> {
    fn new() -> Self {
        Self {
            free_list: array::from_fn(|_| SimpleNonResidentLinkedList::new()),
        }
    }

    fn init<S: PersistentStorageModule>(
        &mut self,
        offset: usize,
        size: usize,
        storage_module: &mut S,
    ) -> Result<(), ()> {
        debug_assert_ne!(size, 0);

        let mut start = offset;

        // add free memory to free list
        let mut end = start + size;

        // make sure the region is properly aligned
        start = (start + size_of::<usize>() - 1) & (!size_of::<usize>() + 1);
        end &= !size_of::<usize>() + 1;
        assert!(start <= end);

        let mut current_start = start;

        while current_start + size_of::<usize>() <= end {
            let size = if current_start == 0 {
                prev_power_of_two(end - current_start)
            } else {
                let lowbit = current_start & (!current_start + 1);
                min(lowbit, prev_power_of_two(end - current_start))
            };

            unsafe {
                self.free_list[size.trailing_zeros() as usize]
                    .push(current_start, storage_module)?;
            }
            current_start += size;
        }

        Ok(())
    }

    fn allocate<S: PersistentStorageModule>(
        &mut self,
        layout: Layout,
        storage_module: &mut S,
    ) -> Result<usize, ()> {
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );
        let class = size.trailing_zeros() as usize;
        for i in class..self.free_list.len() {
            // Find the first non-empty size class
            if !self.free_list[i].is_empty() {
                // Split buffers
                trace!(
                    "Allocate: Have to split {} bucket(s)",
                    (class + 1..i + 1).len()
                );
                for j in (class + 1..i + 1).rev() {
                    if let Some(block) = self.free_list[j].pop(storage_module)? {
                        unsafe {
                            self.free_list[j - 1].push(block + (1 << (j - 1)), storage_module)?;
                            self.free_list[j - 1].push(block, storage_module)?;
                        }
                    } else {
                        return Err(());
                    }
                }

                return Ok(self.free_list[class]
                    .pop(storage_module)?
                    .expect("current block should have free space now"));
            }
        }
        Err(())
    }

    fn deallocate<S: PersistentStorageModule>(
        &mut self,
        offset: usize,
        layout: Layout,
        storage_module: &mut S,
    ) -> Result<(), ()> {
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );
        let class = size.trailing_zeros() as usize;

        // Put back into free list
        unsafe { self.free_list[class].push(offset, storage_module)? };

        // Merge free buddy lists
        let mut current_offset = offset;
        let mut current_class = class;

        while current_class < self.free_list.len() - 1 {
            let buddy = current_offset ^ (1 << current_class);
            let flag =
                self.free_list[current_class]
                    .remove_where(storage_module, true, |block| block == buddy)?
                    > 0;

            // Free buddy found
            if flag {
                self.free_list[current_class].pop(storage_module)?;
                current_offset = min(current_offset, buddy);
                current_class += 1;
                unsafe { self.free_list[current_class].push(current_offset, storage_module)? };

                // newly created class is already greater than previous max class
            } else {
                break;
            }
        }

        Ok(())
    }
}

impl<const ORDER: usize> NonResidentBuddyAllocatorModule<ORDER> {
    #[cfg(feature = "benchmarks")]
    #[allow(unused)]
    pub(crate) fn get_free_list(&self) -> &[SimpleNonResidentLinkedList; ORDER] {
        &self.free_list
    }
    #[cfg(feature = "benchmarks")]
    #[allow(unused)]
    pub(crate) fn get_free_list_mut(&mut self) -> &mut [SimpleNonResidentLinkedList; ORDER] {
        &mut self.free_list
    }
}

fn prev_power_of_two(num: usize) -> usize {
    1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}

#[cfg(test)]
mod test {
    use crate::modules::{
        nonresident_allocator::test::{
            test_non_resident_allocator_simple_generic, AllocatedRegion,
        },
        persistent_storage::PersistentStorageModule,
    };

    use super::NonResidentBuddyAllocatorModule;

    #[test]
    pub(super) fn test_non_resident_allocator_simple() {
        test_non_resident_allocator_simple_generic(check_integrity, "test_non_resident_allocator_simple_buddy");
    }

    /// checks that the free list does not overlap itself
    /// and that it does no overlap with allocated regions
    fn check_integrity<S: PersistentStorageModule>(
        regions: &Vec<AllocatedRegion>,
        allocator: &NonResidentBuddyAllocatorModule<16>,
        storage: &mut S,
    ) {
        let mut items: Vec<AllocatedRegion> = regions.iter().map(|x| x.clone()).collect();
        for i in 0..allocator.free_list.len() {
            let mut iter = allocator.free_list[i].iter();
            while let Some(item) = iter.next(storage).unwrap() {
                items.push(AllocatedRegion {
                    offset: item.get_base_offset(),
                    size: 2usize.pow(i as u32),
                });
            }
        }

    }
}
