extern crate zephyr_sys;

use core::ffi::c_int;

type SPISpec = zephyr_sys::raw::spi_dt_spec;

extern "C" {
    fn mb85rs64v_init(error: *mut c_int) -> SPISpec;
    fn mb85rs64v_validate_id(device: *const SPISpec) -> c_int;
    fn mb85rs64v_write_bytes(device: *const SPISpec, addr: u16, data: *mut u8, num_bytes: u32) -> c_int;
    fn mb85rs64v_read_bytes(device: *const SPISpec, addr: u16, data: *mut u8, num_bytes: u32) -> c_int;
}

pub struct MB85RS64VSpiFram {
    spi_spec: SPISpec
}

impl MB85RS64VSpiFram {
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

impl MB85RS64VSpiFram {
    pub fn read(&self, buffer: &mut [u8], address: u16) -> Result<(), ()> {
        let res = unsafe { mb85rs64v_read_bytes(&self.spi_spec, address, buffer.as_mut_ptr(), buffer.len() as u32) };
        if res != 0 {
            return Err(())
        }

        Ok(())
    }

    pub fn read_byte(&self, address: u16) -> Result<u8, ()> {
        let mut data: u8 = 0;
        let res = unsafe { mb85rs64v_read_bytes(&self.spi_spec, address, &mut data, 1) };

        if res != 0 {
            return Err(());
        }

        Ok(data)
    }

    pub fn write(&self, buffer: &mut [u8], address: u16) -> Result<(), ()> {
        let res = unsafe { mb85rs64v_write_bytes(&self.spi_spec, address, buffer.as_mut_ptr(), buffer.len() as u32) };

        if res != 0 {
            return Err(());
        }

        Ok(())
    }

    pub fn write_byte(&self, mut data: u8, address: u16) -> Result<(), ()> {
        let res = unsafe { mb85rs64v_write_bytes(&self.spi_spec, address, &mut data, 1) };

        if res != 0 {
            return Err(());
        }

        Ok(())
    }
}