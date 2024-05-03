use crate::{
    allocation_identifier::AllocationIdentifier,
    modules::{
        allocator::AllocatorModule,
        nonresident_allocator::{CountedNonResidentLinkedList, NonResidentLinkedList},
        persistent_storage::PersistentStorageModule,
    },
    resident_object::{ResidentObject, ResidentObjectMetadata},
    util::modular_linked_list::ModularLinkedList,
};

type ResidentObjectIdentifier<T> = *mut ResidentObject<T>;

/// Metadata of resident objects that will be saved
/// to non volatile storage, so that program can recover
/// after a power failure
struct ResidentObjectMetadataBackup {
    /// size of the object
    size: usize,

    /// where is this objects stored inside of
    /// persistent storage
    offset: usize,

    /// how many references are there
    ref_cnt: usize,

    /// at which address does this data live
    /// (pointers could exist here so we need to restore
    /// the object at exactly the previous address)
    resident_ptr: usize,
}

impl ResidentObjectMetadataBackup {
    fn new_unused() -> Self {
        ResidentObjectMetadataBackup {
            size: 0,
            offset: 0,
            ref_cnt: 0,
            resident_ptr: 0,
        }
    }

    fn is_unused(&self) -> bool {
        self.resident_ptr == 0
    }
}

pub(crate) struct ResidentObjectManager<A: AllocatorModule> {
    heap: A,

    resident_object_meta_backup: CountedNonResidentLinkedList<ResidentObjectMetadataBackup>,

    resident_list: ModularLinkedList<
        ResidentObjectMetadata,
        fn(&mut ResidentObjectMetadata) -> &mut *mut ResidentObjectMetadata,
    >,
    dirty_list: ModularLinkedList<
        ResidentObjectMetadata,
        fn(&mut ResidentObjectMetadata) -> &mut *mut ResidentObjectMetadata,
    >,
}

impl<A: AllocatorModule> ResidentObjectManager<A> {
    /// Create a new resident object manager
    ///
    /// **Note**: Will overwrite any data, at index 0 of the given persistent storage.
    ///
    /// Returns the newly created instance and the offset from which on data can
    /// be stored to persistent storage safely again.
    pub(crate) fn new<S: PersistentStorageModule>(
        resident_buffer: &mut [u8],
        storage: &mut S,
    ) -> Result<(Self, usize), ()> {
        let mut heap = A::new();
        unsafe {
            heap.init(&mut resident_buffer[0], resident_buffer.len());
        }

        // backup item has to be the first in the persistent storage, so restoring is easier
        let mut meta_backup_list = CountedNonResidentLinkedList::new();
        unsafe { meta_backup_list.push(0, ResidentObjectMetadataBackup::new_unused(), storage) }?;
        let offset =
            CountedNonResidentLinkedList::<ResidentObjectMetadataBackup>::total_item_size();

        let instance = ResidentObjectManager {
            heap: A::new(),
            resident_object_meta_backup: meta_backup_list,
            resident_list: ModularLinkedList::new(ResidentObjectMetadata::get_next_resident_item),
            dirty_list: ModularLinkedList::new(ResidentObjectMetadata::get_next_dirty_item),
        };

        Ok((instance, offset))
    }
}

impl<A: AllocatorModule> ResidentObjectManager<A> {
    pub(crate) fn make_resident<T: Sized>(
        &mut self,
        alloc_id: AllocationIdentifier<T>,
        res_object_id: &ResidentObjectIdentifier<T>,
    ) -> Result<ResidentObjectIdentifier<T>, ()> {
        todo!()
    }

    pub(crate) fn make_nonresident<T: Sized>(
        &mut self,
        alloc_id: AllocationIdentifier<T>,
        resident_object: ResidentObjectIdentifier<T>,
    ) -> Result<(), ()> {
        todo!()
    }
}
