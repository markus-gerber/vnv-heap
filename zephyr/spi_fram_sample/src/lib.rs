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

#![cfg_attr(not(feature = "have_std"), no_std)]

#[macro_use]
extern crate cstr;
#[macro_use]
extern crate log;

extern crate zephyr_macros;
extern crate zephyr;
extern crate zephyr_logger;
extern crate zephyr_core;

use vnv_heap::modules::persistent_storage::PersistentStorageModule;
use spi_fram_storage::MB85RS4MTFramStorageModule;


#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Info);

    let mut ram = unsafe { MB85RS4MTFramStorageModule::new() }.unwrap();

    const LEN: usize = 20000;
    let mut buffer = [0u8; LEN];
    let mut read_buffer = [0u8; LEN];

    for i in 0..buffer.len() {
        let x = (i * 10 + (i % 2) * 5) as u8;
        buffer[i] = x;

        if i % (buffer.len() / (LEN / 100)) == 0 {
            info!("{}/{}", i, buffer.len())
        }
    }

    info!("writing {} bytes...", buffer.len());
    ram.write(0, &buffer).expect("write should be successful");

    info!("reading {} bytes....", buffer.len());
    ram.read(0, &mut read_buffer).expect("read should be successful");

    for i in 0..buffer.len() {
        assert_eq!(read_buffer[i], buffer[i]);
    }
    zephyr_core::any::k_str_out("FRAM Data Test: Success\n");
}
