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
                trace!("Allocate: Have to split {} bucket(s)", (class + 1..i + 1).len());
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
    pub(crate) fn get_free_list(&self) -> &[SimpleNonResidentLinkedList; ORDER] {
        &self.free_list
    }
}

#[cfg(test)]
impl<const ORDER: usize> NonResidentBuddyAllocatorModule<ORDER> {
    /// prints buddy free lists
    fn print_info<S: PersistentStorageModule>(&self, storage: &mut S) {
        let list: Vec<Vec<usize>> = self
            .free_list
            .iter()
            .map(|list| {
                list.iter(storage)
                    .filter(|x| x.is_ok())
                    .map(|x| x.unwrap())
                    .collect()
            })
            .collect();
        println!("{:?}", list);
    }
}

fn prev_power_of_two(num: usize) -> usize {
    1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}

#[cfg(test)]
mod test {
    use core::{alloc::Layout, mem::size_of};

    use crate::modules::{
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        persistent_storage::{test::get_test_storage, PersistentStorageModule},
    };

    use super::NonResidentAllocatorModule;

    #[derive(Debug, Clone, Copy)]
    struct AllocatedRegion {
        offset: usize,
        size: usize,
    }

    fn allocate<const SIZE: usize, S: PersistentStorageModule>(
        size: usize,
        allocator: &mut NonResidentBuddyAllocatorModule<SIZE>,
        regions: &mut Vec<AllocatedRegion>,
        storage: &mut S,
    ) {
        allocator.print_info(storage);
        let offset = allocator
            .allocate(
                Layout::from_size_align(size, size_of::<usize>()).unwrap(),
                storage,
            )
            .expect("should have space left");
        regions.push(AllocatedRegion {
            offset: offset,
            size: size,
        });
    }

    fn deallocate<const SIZE: usize, S: PersistentStorageModule>(
        index: usize,
        allocator: &mut NonResidentBuddyAllocatorModule<SIZE>,
        regions: &mut Vec<AllocatedRegion>,
        storage: &mut S,
    ) -> AllocatedRegion {
        let item = regions.remove(index);
        allocator
            .deallocate(
                item.offset,
                Layout::from_size_align(item.size, size_of::<usize>()).unwrap(),
                storage,
            )
            .unwrap();

        item
    }

    #[test]
    fn test_non_resident_allocator_simple() {
        let mut allocator: NonResidentBuddyAllocatorModule<16> =
            NonResidentBuddyAllocatorModule::new();
        let mut storage = get_test_storage("test_non_resident_allocator_no_overlap", TOTAL_SIZE);
        const MIN_SIZE: usize = size_of::<usize>();
        const TOTAL_SIZE: usize = MIN_SIZE * 4;
        allocator.init(0, TOTAL_SIZE, &mut storage).unwrap();

        let mut regions: Vec<AllocatedRegion> = Vec::new();

        for _ in 0..4 {
            allocate(MIN_SIZE, &mut allocator, &mut regions, &mut storage);
            check_integrity(&regions, &allocator, &mut storage);
        }

        allocator
            .allocate(
                Layout::from_size_align(MIN_SIZE, MIN_SIZE).unwrap(),
                &mut storage,
            )
            .expect_err("should have no space left");

        check_integrity(&regions, &allocator, &mut storage);

        for i in [2, 2, 1, 0] {
            deallocate(i, &mut allocator, &mut regions, &mut storage);
            check_integrity(&regions, &allocator, &mut storage);
        }

        // all items deallocated, should have enough space for big object again
        allocate(TOTAL_SIZE, &mut allocator, &mut regions, &mut storage);
        check_integrity(&regions, &allocator, &mut storage);

        allocator
            .allocate(
                Layout::from_size_align(MIN_SIZE, MIN_SIZE).unwrap(),
                &mut storage,
            )
            .expect_err("should have no space left");
        check_integrity(&regions, &allocator, &mut storage);

        deallocate(0, &mut allocator, &mut regions, &mut storage);
        check_integrity(&regions, &allocator, &mut storage);
    }

    fn check_no_overlap(regions: &Vec<AllocatedRegion>) {
        for (region, i) in regions.iter().zip(0..) {
            for (cmp, j) in regions.iter().zip(0..) {
                if i == j {
                    continue;
                }

                assert!(
                    (cmp.offset + cmp.size <= region.offset)
                        || (region.offset + region.size <= cmp.offset),
                    "allocated regions should not overlap"
                )
            }
        }
    }

    /// checks that the free list does not overlap itself
    /// and that it does no overlap with allocated regions
    fn check_integrity<const ORDER: usize, S: PersistentStorageModule>(
        regions: &Vec<AllocatedRegion>,
        allocator: &NonResidentBuddyAllocatorModule<ORDER>,
        storage: &mut S,
    ) {
        let mut items: Vec<AllocatedRegion> = regions.iter().map(|x| x.clone()).collect();
        for i in 0..allocator.free_list.len() {
            for item in allocator.free_list[i].iter(storage) {
                let item = item.unwrap();
                items.push(AllocatedRegion {
                    offset: item,
                    size: 2usize.pow(i as u32),
                });
            }
        }

        check_no_overlap(&items);
    }
}
