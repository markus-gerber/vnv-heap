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

#![no_std]

#[macro_use]
extern crate log;

extern crate zephyr;
extern crate zephyr_core;
extern crate zephyr_logger;
extern crate zephyr_macros;

use spi_fram_storage::MB85RS4MTFramStorageModule;
use vnv_heap::{
    modules::{
        allocator::LinkedListAllocatorModule,
        nonresident_allocator::NonResidentBuddyAllocatorModule,
        object_management::DefaultObjectManagementModule,
    },
    VNVConfig, VNVHeap,
};

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Trace);

    let storage = unsafe { MB85RS4MTFramStorageModule::new() }.unwrap();

    let config = VNVConfig {
        max_dirty_bytes: 100,
    };
    let mut buffer = [0u8; 100];

    let heap: VNVHeap<
        LinkedListAllocatorModule,
        NonResidentBuddyAllocatorModule<19>,
        DefaultObjectManagementModule,
        MB85RS4MTFramStorageModule,
    > = VNVHeap::new(
        &mut buffer,
        storage,
        LinkedListAllocatorModule::new(),
        config,
        |_, _| {},
    )
    .unwrap();

    let mut obj = heap.allocate::<u32>(10).unwrap();

    {
        let obj_ref = obj.get().unwrap();

        info!("data: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj.get_mut().unwrap();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj.get().unwrap();

        info!("data: {}", *obj_ref);
    }

    let mut obj2 = heap.allocate::<u32>(1000).unwrap();

    {
        let obj_ref = obj2.get().unwrap();

        info!("data2: {}", *obj_ref);
    }

    {
        let mut mut_ref = obj2.get_mut().unwrap();
        *mut_ref += 100;
    }

    {
        let obj_ref = obj2.get().unwrap();

        info!("data2: {}", *obj_ref);
    }

    {
        let obj_ref = obj.get().unwrap();

        info!("data: {}", *obj_ref);
    }
}
