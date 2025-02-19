use core::ptr::null_mut;

use crate::resident_object_manager::resident_object_metadata::ResidentObjectMetadata;

use super::{ObjectStatusWrapper, ObjectManagementModule};


pub struct ClockObjectManagementModule {
    dirty_clock_ptr: *const u8,
    accessed_clock_ptr: *const u8,
}

impl ObjectManagementModule for ClockObjectManagementModule {
    fn new() -> Self {
        Self {
            dirty_clock_ptr: null_mut(),
            accessed_clock_ptr: null_mut()
        }
    }

    fn sync_dirty_data<A: crate::modules::allocator::AllocatorModule, S: crate::modules::persistent_storage::PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        dirty_item_list: super::DirtyItemList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()> {
        let mut iterations = 0;
/*
        let mut iter = dirty_item_list.iter();
        while let Some(mut item) = iter.next() {
            if item.get_ptr() >= self.dirty_clock_ptr {

            }
        }

        loop {
            // only consider pages that not used currently and are valid
            if open_references[self.curr_page] == 0 && valid[self.curr_page] {
                if !self.accessed[self.curr_page] {
                    // page lost its chance, choose it
                    let page = self.curr_page;

                    // update pointer before returning
                    self.curr_page = (self.curr_page + 1) % PAGE_COUNT;
                    return Some(page);
                } else {
                    // page was accessed, give it another chance
                    self.accessed[self.curr_page] = false;
                }
            }

            self.curr_page = (self.curr_page + 1) % PAGE_COUNT;

            if start_page == self.curr_page {
                iterations += 1;
                if iterations == 2 {
                    // we could not find a suitable page
                    return None;
                }
            }
        }
        */
        todo!()
    }

    fn unload_objects<S: crate::modules::persistent_storage::PersistentStorageModule, A: crate::modules::allocator::AllocatorModule>(
        &mut self,
        layout: &std::alloc::Layout,
        resident_item_list: super::ResidentItemList<S, A>,
    ) -> Result<(), ()> {
        todo!()
    }

    fn access_object(&mut self, mut metadata: ObjectStatusWrapper) {
        metadata.access_object();
    }

    fn modify_object(&mut self, mut metadata: ObjectStatusWrapper) {
        metadata.modify_object();
    }
}

