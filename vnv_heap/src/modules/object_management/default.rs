use core::alloc::Layout;

use crate::modules::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule};

use super::{ObjectManagementModule, ResidentIter};

// completely stateless
pub struct DefaultObjectManagementModule;

impl ObjectManagementModule for DefaultObjectManagementModule {
    fn new() -> Self {
        Self
    }

    fn sync_dirty_data<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        mut dirty_item_list: super::DirtyItemList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()> {
        let mut curr: usize = 0;

        let mut iter = dirty_item_list.iter();
        while let Some(mut item) = iter.next() {
            if item.is_user_data_dirty() {
                curr += item.sync_user_data().unwrap_or_default();
                if curr >= required_bytes {
                    return Ok(());
                }
            }
        }

        let mut iter = dirty_item_list.iter();
        while let Some(mut item) = iter.next() {
            if item.is_unused() {
                curr += item.unload().unwrap_or_default();
                if curr >= required_bytes {
                    return Ok(());
                }
            }
        }

        // could not sync enough objects
        Err(())
    }

    fn unload_objects<S: PersistentStorageModule, A: AllocatorModule>(
        &mut self,
        layout: &Layout,
        mut resident_item_list: super::ResidentItemList<S, A>,
    ) -> Result<(), ()> {
        let mut iter: ResidentIter<'_, '_, '_, '_, S, A> = resident_item_list.iter();

        while let Some(item) = iter.next() {
            if let Ok(enough_space) = item.unload_and_check_for_space(layout) {
                if enough_space {
                    // unloaded enough objects to allocate layout
                    return Ok(());
                }
            }
        }

        drop(iter);

        drop(resident_item_list);

        // could not unload enough objects
        Err(())
    }
}
