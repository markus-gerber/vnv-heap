#![cfg_attr(not(feature = "have_std"), no_std)]

#[macro_use]
extern crate cstr;
#[macro_use]
extern crate log;

extern crate zephyr_macros;
extern crate zephyr;
extern crate zephyr_logger;
extern crate zephyr_core;

use mb85rs64v_spi_fram::MB85RS64VSpiFram;

mod mb85rs64v_spi_fram;

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Info);

    let ram = unsafe { MB85RS64VSpiFram::new() }.unwrap();

    const LEN: usize = 512;
    let mut buffer = [0u8; LEN];
    let mut read_buffer = [0u8; LEN];

    for i in 0..buffer.len() {
        let x = (i * 10 + (i % 2) * 5) as u8;
        buffer[i] = x;
    }

    info!("writing {} bytes", buffer.len());
    ram.write(&mut buffer, 0).expect("write should be successful");

    info!("reading {} bytes", read_buffer.len());
    ram.read(&mut read_buffer, 0).expect("read should be successful");

    for i in 0..buffer.len() {
        assert_eq!(read_buffer[i], buffer[i]);
    }
    zephyr_core::any::k_str_out("FRAM Data Test: Success\n");
}
