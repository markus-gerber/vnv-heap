extern crate zephyr_sys;

use core::ffi::c_int;
use core::sync::atomic::AtomicBool;

use vnv_heap::modules::persistent_storage::PersistentStorageModule;

type SPISpec = zephyr_sys::raw::spi_dt_spec;

extern "C" {
    fn mb85rs64v_init(error: *mut c_int) -> SPISpec;
    fn mb85rs64v_validate_id(device: *const SPISpec) -> c_int;
    fn mb85rs64v_write_bytes(device: *const SPISpec, addr: u16, data: *const u8, num_bytes: u32) -> c_int;
    fn mb85rs64v_read_bytes(device: *const SPISpec, addr: u16, data: *mut u8, num_bytes: u32) -> c_int;
}

static ALREADY_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub struct SpiFramStorageModule {
    spi_spec: SPISpec
}

impl SpiFramStorageModule {
    /// You can only create one object of this struct safely
    pub unsafe fn new() -> Result<Self, ()> {
        if ALREADY_INITIALIZED.swap(true, core::sync::atomic::Ordering::SeqCst) {
            // module was already initialized
            // this is not allowed with just one spi spec
            
            // one idea to overcome this for some applications:
            // write new wrapper PersistentStorageModule that manages multiple accesses to
            // this module
            panic!("Creating multiple instances of \"SpiFramStorageModule\" is invalid!");
        }

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

impl Drop for SpiFramStorageModule {
    fn drop(&mut self) {
        ALREADY_INITIALIZED.store(false, core::sync::atomic::Ordering::SeqCst);
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
        
        #[cfg(debug_assertions)]
        let before_hash: u32 = crate::xxhash::xxh32(buffer, 1780281484);

        let res = unsafe { mb85rs64v_write_bytes(&self.spi_spec, address as u16, buffer.as_ptr(), buffer.len() as u32) };

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
        // 8KB
        8192
    }
}