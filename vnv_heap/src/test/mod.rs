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

use crate::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
        persistent_storage::test::get_test_storage,
        persistent_storage::FilePersistentStorageModule
    },
    VNVHeap,
};

mod benchmarks;
mod persist_all;
mod persistency;
mod unload;

#[cfg(not(no_std))]
pub(crate) fn get_test_heap<'a>(
    test_name: &str,
    size: usize,
    resident_buffer: &'a mut [u8],
    dirty_size: usize,
    persist_handler: fn(*mut u8, usize) -> ()
) -> VNVHeap<
    'a,
    LinkedListAllocatorModule,
    NonResidentBuddyAllocatorModule<16>,
    DefaultObjectManagementModule,
    FilePersistentStorageModule
> {
    use crate::VNVConfig;

    let storage = get_test_storage(test_name, size);

    VNVHeap::new(
        resident_buffer,
        storage,
        LinkedListAllocatorModule::new(),
        VNVConfig {
            max_dirty_bytes: dirty_size,
        },
        persist_handler
    )
    .unwrap()
}

#[cfg(no_std)]
pub(crate) fn get_test_heap(
    test_name: &str,
    resident_buffer: &mut [u8],
) -> VNVHeap<LinkedListAllocatorModule, NonResidentBuddyAllocatorModule<16>> {
    panic!("not implemented")
}
