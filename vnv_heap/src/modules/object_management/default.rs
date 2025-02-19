use core::alloc::Layout;

use crate::modules::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule};
use super::ObjectManagementModule;

// completely stateless
pub struct DefaultObjectManagementModule;

impl ObjectManagementModule for DefaultObjectManagementModule {
    fn new() -> Self {
        Self
    }

    fn sync_dirty_data<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        mut list: super::ObjectManagementList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()> {
        let mut curr: usize = 0;

        // STEP 1: Try to sync objects
        let mut iter = list.iter();
        while let Some(mut item) = iter.next() {
            let metadata = item.get_metadata();
            if metadata.is_in_use() && metadata.is_mutable_ref_active() {
                continue;
            }

            if !metadata.is_data_dirty() {
                continue;
            }

            curr += item.sync_user_data().unwrap_or_default();
            if curr >= required_bytes {
                return Ok(());
            }
        }

        // STEP 2: Try to unload objects so that we reduce the amount of metadata (which is currently dirty at all time)
        let mut iter = list.iter();
        while let Some(mut item) = iter.next() {
            let metadata = item.get_metadata();
            if metadata.is_in_use() && metadata.is_mutable_ref_active() {
                continue;
            }

            if metadata.is_in_use() {
                continue;
            }

            curr += item.unload().unwrap_or_default();
            if curr >= required_bytes {
                return Ok(());
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
        let mut iter = list.iter();

        while let Some(mut item) = iter.next() {
            if item.get_metadata().is_in_use() {
                continue;
            }

            if let Ok(enough_space) = item.unload_and_check_for_space(layout) {
                if enough_space {
                    // unloaded enough objects to allocate layout
                    return Ok(());
                }
            }
        }

        drop(iter);
        drop(list);

        // could not unload enough objects
        Err(())
    }
}
