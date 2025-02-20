use super::{
    ObjectManagementIter, ObjectManagementIterItem, ObjectManagementList, ObjectManagementModule,
    ObjectStatusWrapper,
};
use crate::modules::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule};
use core::{alloc::Layout, ptr::null_mut};
use std::marker::PhantomData;

// completely stateless
pub struct ClockObjectManagementModule {
    modified_clock: ModifiedClock,
    resident_clock: ResidentClock,
}

impl ObjectManagementModule for ClockObjectManagementModule {
    fn new() -> Self {
        Self {
            resident_clock: ResidentClock::new(),
            modified_clock: ModifiedClock::new()
        }
    }

    fn sync_dirty_data<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        mut list: super::ObjectManagementList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()> {
        let mut curr: usize = 0;

        // STEP 1: Try to sync objects
        let mut iter = self.modified_clock.iter(&mut list);
        while let Some(mut sub_iter) = iter.next() {
            while let Some(mut item) = sub_iter.next() {
                curr += item.sync_user_data().unwrap_or_default();
                if curr >= required_bytes {
                    return Ok(());
                }
            }
        }

        // STEP 2: Try to unload objects so that we reduce the amount of metadata (which is currently dirty at all time)
        let mut iter = self.resident_clock.iter(&mut list);
        while let Some(mut sub_iter) = iter.next() {
            while let Some(item) = sub_iter.next() {
                curr += item.unload().unwrap_or_default();
                if curr >= required_bytes {
                    return Ok(());
                }
            }
        }

        // could not sync enough objects
        Err(())
    }

    fn unload_objects<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        layout: &Layout,
        mut list: super::ObjectManagementList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()> {
        let mut iter = self.resident_clock.iter(&mut list);
        while let Some(mut sub_iter) = iter.next() {
            while let Some(item) = sub_iter.next() {
                if let Ok(enough_space) = item.unload_and_check_for_space(layout) {
                    if enough_space {
                        // unloaded enough objects to allocate layout
                        return Ok(());
                    }
                }
            }
        }

        // could not unload enough objects
        Err(())
    }

    fn access_object(&mut self, mut metadata: ObjectStatusWrapper) {
        metadata.access_object();
    }

    fn modify_object(&mut self, mut metadata: ObjectStatusWrapper) {
        metadata.modify_object();
    }
}

struct ModifiedClock {
    curr_ptr: *const u8,
}

impl ModifiedClock {
    pub fn new() -> Self {
        Self { curr_ptr: null_mut() }
    }
}
impl GenericClock for ModifiedClock {
    fn get_curr_ptr(&mut self) -> &mut *const u8 {
        &mut self.curr_ptr
    }

    fn get_clock_bit<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
    ) -> bool {
        item.get_metadata().was_modified()
    }

    fn set_clock_bit<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
        is_set: bool,
    ) {
        item.get_metadata().set_was_modified(is_set);
    }

    fn iter_item_valid<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
    ) -> bool {
        if !item.get_metadata().is_data_dirty() {
            return false;
        }
    
        !item.get_metadata().is_mutable_ref_active()
    }
}
struct ResidentClock {
    curr_ptr: *const u8,
}

impl ResidentClock {
    pub fn new() -> Self {
        Self { curr_ptr: null_mut() }
    }
}

impl GenericClock for ResidentClock {
    fn get_curr_ptr(&mut self) -> &mut *const u8 {
        &mut self.curr_ptr
    }

    fn get_clock_bit<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
    ) -> bool {
        item.get_metadata().was_accessed()
    }

    fn set_clock_bit<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
        is_set: bool,
    ) {
        item.get_metadata().set_was_accessed(is_set);
    }

    fn iter_item_valid<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
    ) -> bool {
        !item.get_metadata().is_in_use()
    }
}

/// Generic clock algorithm. Usable for both access/unload and modify/sync operations.
trait GenericClock: Sized {
    fn get_curr_ptr(&mut self) -> &mut *const u8;

    fn get_clock_bit<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
    ) -> bool;
    fn set_clock_bit<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
        is_set: bool,
    );

    /// Used as a precondition for the items considered in `iter()`
    fn iter_item_valid<A: AllocatorModule, S: PersistentStorageModule>(
        item: &mut ObjectManagementIterItem<A, S>,
    ) -> bool;

    fn iter<'a, 'b, 'c, 'd, 'e, A: AllocatorModule, S: PersistentStorageModule>(
        &'e mut self,
        list: &'e mut ObjectManagementList<'a, 'b, 'c, 'd, A, S>,
    ) -> GenericClockIter<'a, 'b, 'c, 'd, 'e, A, S, Self> {
        let ptr = self.get_curr_ptr();
        GenericClockIter {
            start_ptr: *ptr,
            curr_ptr: ptr,
            list,
            iteration_cnt: 0,
            _phantom_data: PhantomData,
        }
    }

    fn access_object<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        item: &mut ObjectManagementIterItem<A, S>,
    ) {
        Self::set_clock_bit(item, true);
    }
}

struct GenericClockSubIter<
    'a,
    'b,
    'c,
    'd,
    'e,
    A: AllocatorModule,
    S: PersistentStorageModule,
    C: GenericClock,
> {
    internal_iter: ObjectManagementIter<'a, 'b, 'c, 'd, A, S>,
    iteration_cnt: &'e u8,
    start_ptr: &'e *const u8,
    curr_ptr: &'e mut *const u8,
    _phantom_data: PhantomData<C>,
}

impl<'a, 'b, 'c, A: AllocatorModule, S: PersistentStorageModule, C: GenericClock>
    GenericClockSubIter<'a, 'b, '_, 'c, '_, A, S, C>
{
    fn next<'d>(&'d mut self) -> Option<ObjectManagementIterItem<'a, 'b, '_, '_, 'c, 'd, A, S>> {
        while let Some(item) = self.internal_iter.next() {
            let mut item: ObjectManagementIterItem<'_, '_, '_, '_, 'c, 'd, A, S> =
                unsafe { core::mem::transmute(item) };


            if *self.iteration_cnt == 1 && item.get_ptr() < *self.start_ptr {
                // first iteration: skip to the start pointer
                continue;
            }

            if Self::handle(&mut item) {
                // item is accepted, update current pointer and return
                *self.curr_ptr = item.get_ptr();
                return Some(item);
            }
        }

        // end of iteration, reset curr pointer and return
        *self.curr_ptr = null_mut();
        return None;
    }

    /// Handles the current item. Returns true if the item is accepted, false otherwise.
    /// 
    /// This function also manages the clock bit.
    fn handle(item: &mut ObjectManagementIterItem<'a, 'b, '_, '_, 'c, '_, A, S>) -> bool {
        if C::iter_item_valid(item) {
            if !C::get_clock_bit(item) {
                // object lost its chance, choose it
                return true;
            } else {
                // give this object another chance
                C::set_clock_bit(item, false);
            }
        }
        return false;
    }
}

struct GenericClockIter<
    'a,
    'b,
    'c,
    'd,
    'e,
    A: AllocatorModule,
    S: PersistentStorageModule,
    C: GenericClock,
> {
    start_ptr: *const u8,
    curr_ptr: &'e mut *const u8,
    list: &'e mut ObjectManagementList<'a, 'b, 'c, 'd, A, S>,
    iteration_cnt: u8,
    _phantom_data: PhantomData<C>,
}

impl<'a, 'b, 'e, A: AllocatorModule, S: PersistentStorageModule, C: GenericClock>
    GenericClockIter<'a, 'b, '_, '_, 'e, A, S, C>
{
    fn next(&mut self) -> Option<GenericClockSubIter<'a, 'b, '_, '_, '_, A, S, C>> {
        // first iteration:  get to the right pointer + start first real iteration
        // second iteration: end first real iteration (until start pointer) + start second real iteration
        // third iteration:  end second real iteration (until start pointer)
        if self.iteration_cnt == 3 {
            // cancel fourth iteration
            return None;
        }

        self.iteration_cnt += 1;
        if self.iteration_cnt == 1 && self.start_ptr.is_null() {
            // skip the first iteration and start the second one
            self.iteration_cnt += 1;
        }

        Some(GenericClockSubIter {
            internal_iter: self.list.iter(),
            start_ptr: &self.start_ptr,
            iteration_cnt: &self.iteration_cnt,
            curr_ptr: self.curr_ptr,
            _phantom_data: PhantomData,
        })
    }
}

#[cfg(test)]
mod test {
    use std::sync::atomic::AtomicBool;

    use try_lock::TryLock;

    use crate::{allocation_identifier::AllocationIdentifier, modules::{allocator::LinkedListAllocatorModule, object_management::{clock::GenericClock, ObjectManagementList, ObjectManagementListArguments}, persistent_storage::test::get_test_storage}, resident_object_manager::{resident_list::ResidentList, resident_object_metadata::ResidentObjectMetadata, ResidentObjectManager}, shared_persist_lock::SharedPersistLock};

    use super::ClockObjectManagementModule;

    #[test]
    fn test_clock_object_management_module_flush() {
        const OBJ_SIZE: usize = 8;
        type Object = [u8; OBJ_SIZE];

        const BUFFER_SIZE: usize = 2 * 1024;
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut allocator = LinkedListAllocatorModule::new();
        let persist_queued = AtomicBool::new(false);
        let allocator_lock = TryLock::new(());
        let shared_allocator = SharedPersistLock::new(&mut allocator as *mut LinkedListAllocatorModule, &persist_queued, &allocator_lock);

        let mut resident_list = ResidentList::new();

        let mut resident_object_manager = ResidentObjectManager::<_, ClockObjectManagementModule>::new(&mut buffer, BUFFER_SIZE, &mut resident_list, shared_allocator).unwrap();
        let mut storage = get_test_storage("test_clock_object_management_module_flush", 4 * 1024);
        
        let mut allocated_objects = vec![];
        let mut allocated_objects_is_dirty = vec![];
        let mut allocated_objects_is_resident = vec![];
        let mut allocated_objects_clock_dirty = vec![];
        let mut allocated_objects_clock_resident = vec![];
        
        let mut curr_alloc_offset = 0; // offset where new data can be allocated on storage

        fn find_element_mut<T: Sized>(
            manager: &mut ResidentObjectManager<LinkedListAllocatorModule, ClockObjectManagementModule>,
            alloc_id: &AllocationIdentifier<T>,
        ) -> Option<*mut ResidentObjectMetadata> {
            let mut iter = manager.resident_list.iter_mut();
            while let Some(mut item) = iter.next() {
                let item_ref = item.get_element();
                if item_ref.inner.offset == alloc_id.offset {
                    return Some(item_ref);
                }
            }
            None
        }

        macro_rules! construct_management_args {
            () => {
                {
                    let args = ObjectManagementListArguments {
                        allocator: &mut resident_object_manager.heap,
                        remaining_dirty_size: &mut resident_object_manager.remaining_dirty_size,
                        storage: &mut storage
                    };
                    args
                }
                
            };
        }
        macro_rules! construct_management_list {
            ($args: ident) => {
                {
                    let list = ObjectManagementList{
                        arguments: &mut $args,
                        resident_list: &mut resident_object_manager.resident_list,
                    };
                    list
                }
            };
        }

        macro_rules! check_integrity {
            () => {
                for (i, obj) in allocated_objects.iter().enumerate() {
                    assert_eq!(allocated_objects_is_resident[i], resident_object_manager.is_resident(obj), "object with offset {} does not match expected resident state", obj.offset);
                    if allocated_objects_is_resident[i] {
                        assert_eq!(allocated_objects_is_dirty[i], resident_object_manager.is_data_dirty(obj), "object with offset {} does not match expected dirty state", obj.offset);
                        let element = unsafe { find_element_mut(&mut resident_object_manager, &allocated_objects[i]).unwrap().as_mut().unwrap() };
                        assert_eq!(element.inner.status.is_clock_accessed_bit_set(), allocated_objects_clock_resident[i], "object with offset {} does not match expected clock accessed state", obj.offset);
                        assert_eq!(element.inner.status.is_clock_modified_bit_set(), allocated_objects_clock_dirty[i], "object with offset {} does not match expected clock accessed state", obj.offset);
                    }
                }
            };
        }

        // PART 1: Allocate some dummy objects
        // All of them should be dirty and resident
        for _ in 0..10 {
            let offset = curr_alloc_offset;
            curr_alloc_offset += OBJ_SIZE;

            allocated_objects.push(AllocationIdentifier::<Object>::from_offset(offset));
            allocated_objects_is_dirty.push(true);
            allocated_objects_is_resident.push(true);
            allocated_objects_clock_dirty.push(true);
            allocated_objects_clock_resident.push(true);

            resident_object_manager.try_to_allocate::<Object>(Default::default(), offset, false).unwrap();
        }

        check_integrity!();

        // PART 2: Flush two objects
        // Except that first object is flushed and clock bits for all other objects are set to 0 (because of two full rotations

        {
            let manager = &mut resident_object_manager.object_manager;
            let mut args = construct_management_args!();
            let mut list = construct_management_list!(args);
            let mut  iter = manager.modified_clock.iter(&mut list);
            let mut sub_iter = iter.next().unwrap();
            assert!(sub_iter.next().is_none());

            assert_eq!(*sub_iter.iteration_cnt, 2);
            
            let mut sub_iter = iter.next().unwrap();
            let mut next = sub_iter.next().unwrap();
            assert_eq!(next.delete_handle.get_element().inner.offset, allocated_objects[0].offset);
            next.sync_user_data().unwrap();
            
            let mut next = sub_iter.next().unwrap();
            assert_eq!(next.delete_handle.get_element().inner.offset, allocated_objects[1].offset);
            next.sync_user_data().unwrap();
        }

        allocated_objects_is_dirty[0] = false;
        allocated_objects_is_dirty[1] = false;
        allocated_objects_clock_dirty = allocated_objects_clock_dirty.iter().map(|_| false).collect();

        check_integrity!();

        // PART 3: Temporarily get mut for all elements 3..6 and 7..10 and get mut for 2 permanently

        for i in 2..10 {
            if i == 6 {
                continue;
            }

            unsafe {
                resident_object_manager.get_mut(&allocated_objects[i], false, &mut storage).unwrap();

                if i != 2 {
                    resident_object_manager.release_mut(&allocated_objects[i]);
                }
            }
        }

        {
            let manager = &mut resident_object_manager.object_manager;
            let mut args = construct_management_args!();
            let mut list = construct_management_list!(args);
            let mut  iter = manager.modified_clock.iter(&mut list);
            let mut sub_iter = iter.next().unwrap();
            assert_eq!(*sub_iter.iteration_cnt, 1);

            let mut next = sub_iter.next().unwrap();
            assert_eq!(next.delete_handle.get_element().inner.offset, allocated_objects[6].offset);
            next.sync_user_data().unwrap();

            assert!(sub_iter.next().is_none());

            let mut sub_iter = iter.next().unwrap();
            let mut next = sub_iter.next().unwrap();
            assert_eq!(next.delete_handle.get_element().inner.offset, allocated_objects[3].offset);
            next.sync_user_data().unwrap();
        }

        allocated_objects_is_dirty[6] = false;
        allocated_objects_is_dirty[3] = false;
        allocated_objects_clock_dirty = vec![false, false, true, false, false, false, false, false, false, false];

        check_integrity!();

        macro_rules! single_sync {
            ($expected_iteration: literal, $excepted_offset: literal) => {
                let manager = &mut resident_object_manager.object_manager;
                let mut args = construct_management_args!();
                let mut list = construct_management_list!(args);
                let mut  iter = manager.modified_clock.iter(&mut list);
                for _ in 0..($expected_iteration-1) {
                    let mut sub_iter = iter.next().expect("no sub iter for skip");
                    assert!(sub_iter.next().is_none());
                }
                let mut sub_iter = iter.next().expect("no sub iter");
                let mut next = sub_iter.next().expect("no next");
                assert_eq!(next.delete_handle.get_element().inner.offset, allocated_objects[$excepted_offset].offset);
                next.sync_user_data().unwrap();
                allocated_objects_is_dirty[$excepted_offset] = false;
                check_integrity!();
            };
        }

        unsafe { resident_object_manager.release_mut(&allocated_objects[2]) };
        unsafe {
            let x = find_element_mut(&mut resident_object_manager, &allocated_objects[2]).unwrap().as_mut().unwrap();
            assert!(!x.inner.status.is_in_use());
            assert!(!x.inner.status.is_mutable_ref_active());
        }

        single_sync!(1, 4);
        single_sync!(1, 5);
        single_sync!(1, 7);
        single_sync!(1, 8);
        single_sync!(1, 9);
        allocated_objects_clock_dirty[2] = false;
        single_sync!(3, 2);

        {
            let manager = &mut resident_object_manager.object_manager;
            let mut args = construct_management_args!();
            let mut list = construct_management_list!(args);
            let mut  iter = manager.modified_clock.iter(&mut list);
            for _ in 0..3 {
                let mut sub_iter = iter.next().expect("no sub iter for skip");
                assert!(sub_iter.next().is_none());
            }
        }
    }


    #[test]
    fn test_clock_object_management_module_unload() {
        const OBJ_SIZE: usize = 256;
        type Object = [u8; OBJ_SIZE];

        const BUFFER_SIZE: usize = 1024;
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut allocator = LinkedListAllocatorModule::new();
        let persist_queued = AtomicBool::new(false);
        let allocator_lock = TryLock::new(());
        let shared_allocator = SharedPersistLock::new(&mut allocator as *mut LinkedListAllocatorModule, &persist_queued, &allocator_lock);

        let mut resident_list = ResidentList::new();

        let mut resident_object_manager = ResidentObjectManager::<_, ClockObjectManagementModule>::new(&mut buffer, BUFFER_SIZE, &mut resident_list, shared_allocator).unwrap();
        let mut storage = get_test_storage("test_clock_object_management_module_unload", 4 * 1024);
        
        let mut allocated_objects = vec![];        
        let mut curr_alloc_offset = 0; // offset where new data can be allocated on storage

        let mut allocated_objects_is_resident = vec![];

        macro_rules! check_integrity {
            () => {
                for (i, obj) in allocated_objects.iter().enumerate() {
                    assert_eq!(allocated_objects_is_resident[i], resident_object_manager.is_resident(obj), "object with offset {} does not match expected resident state", obj.offset);
                }
            };
        }


        for _ in 0..10 {
            let offset = curr_alloc_offset;
            curr_alloc_offset += OBJ_SIZE;
            let identifier = AllocationIdentifier::<Object>::from_offset(offset);
            allocated_objects.push(identifier.clone());
            allocated_objects_is_resident.push(false);

        }
        
        for i in 0..3 {
            let identifier = &allocated_objects[i];
            unsafe {
                resident_object_manager.get_ref::<Object, _>(&identifier, false, &mut storage).unwrap();

                if i != 0 {
                    resident_object_manager.release_ref(&identifier);
                }
            };
            allocated_objects_is_resident[i] = true;
        }

        check_integrity!();
        unsafe {
            resident_object_manager.get_ref::<Object, _>(&allocated_objects[3], false, &mut storage).unwrap();
            resident_object_manager.release_ref(&allocated_objects[3]);
        };
        allocated_objects_is_resident[3] = true;
        allocated_objects_is_resident[1] = false;
        check_integrity!();
        
    }
}
