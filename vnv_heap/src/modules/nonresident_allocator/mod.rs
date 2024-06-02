use super::persistent_storage::PersistentStorageModule;
use core::alloc::Layout;

mod buddy;
mod linked_list;

pub use buddy::NonResidentBuddyAllocatorModule;
pub use linked_list::{
    Iter, SimpleIter, NonResidentLinkedList, SimpleNonResidentLinkedList, AtomicPushOnlyNonResidentLinkedList, SharedAtomicLinkedListHeadPtr
};

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
