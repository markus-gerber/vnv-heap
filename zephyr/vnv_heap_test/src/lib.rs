
#[macro_use]
extern crate cstr;
#[macro_use]
extern crate log;

extern crate zephyr_macros;
extern crate zephyr;
extern crate zephyr_logger;
extern crate zephyr_core;

mod test;

use test::test_heap_persistency;

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Debug);

    test_heap_persistency();
    log::debug!("#############################");
    log::debug!("### TESTS WERE SUCCESSFUL ###");
    log::debug!("#############################");
}
