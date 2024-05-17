
extern crate zephyr_macros;
extern crate zephyr;
extern crate zephyr_logger;
extern crate zephyr_core;

mod test;

use test::test_heap_persistency;

#[no_mangle]
pub extern "C" fn rust_main() {
    zephyr_logger::init(log::LevelFilter::Warn);

    test_heap_persistency();
    log::warn!("#############################");
    log::warn!("### TESTS WERE SUCCESSFUL ###");
    log::warn!("#############################");
}
