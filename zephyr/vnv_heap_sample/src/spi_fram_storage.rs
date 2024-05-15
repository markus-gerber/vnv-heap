extern crate zephyr_sys;

use core::ffi::c_int;

use vnv_heap::modules::persistent_storage::PersistentStorageModule;

type SPISpec = zephyr_sys::raw::spi_dt_spec;

extern "C" {
    fn mb85rs64v_init(error: *mut c_int) -> SPISpec;
    fn mb85rs64v_validate_id(device: *const SPISpec) -> c_int;
    fn mb85rs64v_write_bytes(device: *const SPISpec, addr: u16, data: *const u8, num_bytes: u32) -> c_int;
    fn mb85rs64v_read_bytes(device: *const SPISpec, addr: u16, data: *mut u8, num_bytes: u32) -> c_int;
}

pub struct SpiFramStorageModule {
    spi_spec: SPISpec
}

impl SpiFramStorageModule {
    /// You can only create one object of this struct safely
    pub unsafe fn new() -> Result<Self, ()> {
        let mut result: c_int = 0;
        let spec = mb85rs64v_init(&mut result);
        if result != 0 {
            return Err(());
        }

        if mb85rs64v_validate_id(&spec) != 0 {
            return Err(());
        }

        Ok(Self {
            spi_spec: spec
        })
    }
}

impl PersistentStorageModule for SpiFramStorageModule {
    fn read(&mut self, address: usize, buffer: &mut [u8]) -> Result<(), ()> {
        debug_assert!(address <= (u16::MAX as usize));

        let res = unsafe { mb85rs64v_read_bytes(&self.spi_spec, address as u16, buffer.as_mut_ptr(), buffer.len() as u32) };
        if res != 0 {
            return Err(())
        }

        Ok(())
    }

    fn write(&mut self, address: usize, buffer: &[u8]) -> Result<(), ()> {
        debug_assert!(address <= (u16::MAX as usize));

        let res = unsafe { mb85rs64v_write_bytes(&self.spi_spec, address as u16, buffer.as_ptr(), buffer.len() as u32) };

        if res != 0 {
            return Err(());
        }

        Ok(())
    }

    fn get_max_size(&self) -> usize {
        // 8KB
        8192
    }
}