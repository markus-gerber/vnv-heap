/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

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
