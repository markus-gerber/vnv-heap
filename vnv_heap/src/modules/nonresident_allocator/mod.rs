use super::persistent_storage::PersistentStorageModule;
use core::alloc::Layout;

mod block;
mod buddy;
mod linked_list;

pub use buddy::NonResidentBuddyAllocatorModule;
pub use linked_list::{
    AtomicPushOnlyNonResidentLinkedList, Iter, NonResidentLinkedList,
    SharedAtomicLinkedListHeadPtr, SimpleIter, SimpleNonResidentLinkedList,
};
pub use block::{NonResidentBlockAllocator, calc_non_resident_block_allocator_bit_list_size};

/// An allocator module that is not stored inside RAM,
/// but is rather stored on some kind of non volatile storage device
pub trait NonResidentAllocatorModule {
    /// Creates a new allocator module object.
    ///
    /// **Note**: It first will be initialized before it will be used
    fn new() -> Self;

    /// Initializes the allocator module with a given memory area
    /// `[0, size)` inside of `storage_module`.
    fn init<S: PersistentStorageModule>(
        &mut self,
        offset: usize,
        size: usize,
        storage_module: &mut S,
    ) -> Result<(), ()>;

    /// Allocates new memory and returns the `offset` where the data can be placed
    fn allocate<S: PersistentStorageModule>(
        &mut self,
        layout: Layout,
        storage_module: &mut S,
    ) -> Result<usize, ()>;

    /// Deallocates memory
    fn deallocate<S: PersistentStorageModule>(
        &mut self,
        offset: usize,
        layout: Layout,
        storage_module: &mut S,
    ) -> Result<(), ()>;
}

#[cfg(test)]
mod test {
    use core::{alloc::Layout, mem::size_of};

    use crate::modules::persistent_storage::{
        test::get_test_storage, FilePersistentStorageModule, PersistentStorageModule,
    };

    use super::NonResidentAllocatorModule;

    #[derive(Debug, Clone, Copy)]
    pub(super) struct AllocatedRegion {
        pub(super) offset: usize,
        pub(super) size: usize,
    }

    fn allocate<S: PersistentStorageModule, N: NonResidentAllocatorModule>(
        size: usize,
        allocator: &mut N,
        regions: &mut Vec<AllocatedRegion>,
        storage: &mut S,
    ) {
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

    fn deallocate<S: PersistentStorageModule, N: NonResidentAllocatorModule>(
        index: usize,
        allocator: &mut N,
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

    pub(super) fn test_non_resident_allocator_simple_generic<N: NonResidentAllocatorModule>(
        check_integrity: fn(
            regions: &Vec<AllocatedRegion>,
            allocator: &N,
            storage: &mut FilePersistentStorageModule,
        ),
        name: &'static str
    ) {
        let mut allocator: N = N::new();
        const MIN_SIZE: usize = size_of::<usize>();
        const TOTAL_SIZE: usize = 1024;

        let mut storage = get_test_storage(name, TOTAL_SIZE);
        allocator.init(0, TOTAL_SIZE, &mut storage).unwrap();

        let mut regions: Vec<AllocatedRegion> = Vec::new();

        macro_rules! check_integrity_all {
            () => {
                check_no_overlap(&regions);
                check_integrity(&regions, &allocator, &mut storage);
            };
        }

        for _ in 0..4 {
            allocate(MIN_SIZE, &mut allocator, &mut regions, &mut storage);
            check_integrity_all!();
        }

        deallocate(2, &mut allocator, &mut regions, &mut storage);
        
        allocate(MIN_SIZE * 2, &mut allocator, &mut regions, &mut storage);
        check_integrity_all!();

        allocate(MIN_SIZE, &mut allocator, &mut regions, &mut storage);
        check_integrity_all!();

        allocate(MIN_SIZE * 4, &mut allocator, &mut regions, &mut storage);
        check_integrity_all!();

        for i in [2, 3, 1, 0, 1, 0] {
            deallocate(i, &mut allocator, &mut regions, &mut storage);
            check_integrity_all!();
        }

        // all items deallocated, should have enough space for big object again
        allocate(TOTAL_SIZE, &mut allocator, &mut regions, &mut storage);
        check_integrity_all!();

        allocator
            .allocate(
                Layout::from_size_align(MIN_SIZE, MIN_SIZE).unwrap(),
                &mut storage,
            )
            .expect_err("should have no space left");
        check_integrity_all!();

        deallocate(0, &mut allocator, &mut regions, &mut storage);
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
}
