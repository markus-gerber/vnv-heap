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

extern crate zephyr_sys;

use core::ffi::c_int;
use core::sync::atomic::AtomicBool;

use vnv_heap::modules::persistent_storage::PersistentStorageModule;

type SPISpec = zephyr_sys::raw::spi_dt_spec;

extern "C" {
    fn mb85rs4mt_init(error: *mut c_int) -> SPISpec;
    fn mb85rs4mt_validate_id(device: *const SPISpec) -> c_int;
    fn mb85rs4mt_write_bytes(device: *const SPISpec, addr: u32, data: *const u8, num_bytes: u32) -> c_int;
    fn mb85rs4mt_read_bytes(device: *const SPISpec, addr: u32, data: *mut u8, num_bytes: u32) -> c_int;
}

static ALREADY_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub struct MB85RS4MTFramStorageModule {
    spi_spec: SPISpec
}

impl MB85RS4MTFramStorageModule {
    /// You can only create one object of this struct safely
    pub unsafe fn new() -> Result<Self, ()> {
        if ALREADY_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst) {
            // module was already initialized
            // this is not allowed with just one spi spec
            
            // one idea to overcome this for some applications:
            // write new wrapper PersistentStorageModule that manages multiple accesses to
            // this module
            panic!("Creating multiple instances of \"MB85RS4MTFramStorageModule\" is invalid!");
        }

        let mut result: c_int = 0;
        let spec = mb85rs4mt_init(&mut result);
        if result != 0 {
            return Err(());
        }

        if mb85rs4mt_validate_id(&spec) != 0 {
            return Err(());
        }

        Ok(Self {
            spi_spec: spec
        })
    }
}

impl Drop for MB85RS4MTFramStorageModule {
    fn drop(&mut self) {
        ALREADY_INITIALIZED.store(false, core::sync::atomic::Ordering::SeqCst);
    }
}

impl PersistentStorageModule for MB85RS4MTFramStorageModule {
    fn read(&mut self, address: usize, buffer: &mut [u8]) -> Result<(), ()> {
        debug_assert!(address + buffer.len() <= self.get_max_size());

        let res = unsafe { mb85rs4mt_read_bytes(&self.spi_spec, address as u32, buffer.as_mut_ptr(), buffer.len() as u32) };
        if res != 0 {
            return Err(())
        }

        Ok(())
    }

    fn write(&mut self, address: usize, buffer: &[u8]) -> Result<(), ()> {
        debug_assert!(address + buffer.len() <= self.get_max_size());
        
        #[cfg(debug_assertions)]
        let before_hash: u32 = crate::xxhash::xxh32(buffer, 1780281484);

        let res = unsafe { mb85rs4mt_write_bytes(&self.spi_spec, address as u32, buffer.as_ptr(), buffer.len() as u32) };

        #[cfg(debug_assertions)]
        {
            let after_hash: u32 = crate::xxhash::xxh32(buffer, 1780281484);
            debug_assert_eq!(before_hash, after_hash, "buffer should not change when writing bytes!");
        }

        if res != 0 {
            return Err(());
        }

        Ok(())
    }

    fn get_max_size(&self) -> usize {
        // 512KB
        524288
    }
}