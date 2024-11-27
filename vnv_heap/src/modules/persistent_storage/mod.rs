mod access_distribution;
pub(crate) use access_distribution::*;

#[cfg(not(no_std))]
mod file_storage;

#[cfg(not(no_std))]
pub use file_storage::FilePersistentStorageModule;


pub trait PersistentStorageModule {
    /// Reads a region `[offset, offset + dest.len())` to a storage location `dest` that is at least `dest.len()` bytes big.
    ///
    /// If this call fails, it could be that already some data was written to `dest`.
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()>;

    /// Returns the maximum size in bytes of this storage
    ///
    /// **Although `read` and `write` won't throw any error, it is illegal to read/write across this border!**
    fn get_max_size(&self) -> usize;

    /// Writes the region `src` back to the underlying storage `[offset, offset + size.len()]`
    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()>;

    /// A function that can be used to tell underlying caching layers that the region `[offset, size)`
    /// will probably not be accessed in the near future.
    ///
    /// (So you probably only want to overwrite this function if you are defining a cache)
    fn forget_region(&mut self, _offset: usize, _size: usize) {}
}

pub(crate) mod persistent_storage_util {
    use core::{mem::{size_of, MaybeUninit}, ptr::{slice_from_raw_parts, slice_from_raw_parts_mut}};

    use super::PersistentStorageModule;

    #[inline]
    pub(crate) fn write_storage_data<T: Sized, P: PersistentStorageModule>(storage: &mut P, offset: usize, src: &T) -> Result<(), ()> {
        let buffer = slice_from_raw_parts((src as *const T) as *mut u8, size_of::<T>());
        storage.write(offset, unsafe { buffer.as_ref().unwrap() })?;

        Ok(())
    }

    #[inline]
    pub(crate) unsafe fn read_storage_data_into<T: Sized, P: PersistentStorageModule>(storage: &mut P, offset: usize, dest: &mut T) -> Result<(), ()> {
        let buffer = slice_from_raw_parts_mut((dest as *mut T) as *mut u8, size_of::<T>());
        storage.read(offset, buffer.as_mut().unwrap())?;

        Ok(())
    }

    #[inline]
    pub(crate) unsafe fn read_storage_data<T: Sized, P: PersistentStorageModule>(storage: &mut P, offset: usize) -> Result<T, ()> {
        let mut res: MaybeUninit<T> = MaybeUninit::uninit();
        read_storage_data_into(storage, offset, &mut res)?;

        Ok(res.assume_init())
    }
}

#[cfg(test)]
pub(crate) mod test {
    use crate::modules::persistent_storage::persistent_storage_util::{read_storage_data, read_storage_data_into, write_storage_data};

    use super::{FilePersistentStorageModule, PersistentStorageModule};
    use core::mem::size_of;

    #[cfg(not(no_std))]
    pub(crate) fn get_test_storage(test_name: &str, size: usize) -> FilePersistentStorageModule {
        FilePersistentStorageModule::new(format!("/tmp/{}.tmp", test_name), size).unwrap()
    }

    // implement for other test targets, also: change to right return type
    #[cfg(no_std)]
    pub(crate) fn get_test_storage(test_name: &str, size: usize) -> () {
        panic!("not implemented")
    }

    fn gen_number(i: usize) -> u8 {
        (i * 3 + (i % 3) * 7 + (i % 11) * 51) as u8
    }

    pub(super) const PERSISTENT_STORAGE_NORMAL_TEST_SIZE: usize = 4096;

    /// test if write saves all data and read restores all of it
    pub(super) fn test_persistent_storage_normal<T: PersistentStorageModule>(mut module: T) {
        const SUB_TEST_SIZE: usize = PERSISTENT_STORAGE_NORMAL_TEST_SIZE / 32;

        // generate some random data
        let mut source_slice = [0u8; PERSISTENT_STORAGE_NORMAL_TEST_SIZE];
        for i in 0..PERSISTENT_STORAGE_NORMAL_TEST_SIZE {
            source_slice[i] = gen_number(i as usize);
        }

        let mut test_slice = [0u8; SUB_TEST_SIZE];

        for i in 0..PERSISTENT_STORAGE_NORMAL_TEST_SIZE / SUB_TEST_SIZE {
            let offset = i * SUB_TEST_SIZE;

            for x in 0..SUB_TEST_SIZE {
                test_slice[x] = source_slice[offset + x];
            }

            module.write(offset, &test_slice).unwrap();
        }

        for i in 0..PERSISTENT_STORAGE_NORMAL_TEST_SIZE / SUB_TEST_SIZE {
            let offset = i * SUB_TEST_SIZE;
            module.read(offset, &mut test_slice).unwrap();

            for x in 0..SUB_TEST_SIZE {
                assert_eq!(test_slice[x], source_slice[offset + x]);
            }
        }
    }

    pub(super) const PERSISTENT_STORAGE_CUSTOM_TYPE_TEST_SIZE: usize = 100;

    pub(super) fn test_persistent_storage_custom_type<T: PersistentStorageModule>(mut module: T) {
        module.write(0, &[255u8; 100]).unwrap();

        #[derive(PartialEq, Debug)]
        struct TestData {
            a: usize,
            b: bool,
            c: Option<u16>,
        }

        let mut original = TestData {
            a: 21,
            b: true,
            c: None,
        };

        write_storage_data(&mut module, 10, &original).unwrap();

        // make sure that data was only written to that area
        let mut test_buffer = [0u8; 100];
        module.read(0, &mut test_buffer).unwrap();

        for i in 0..test_buffer.len() {
            if !(i >= 10 && i < 10 + size_of::<TestData>()) {
                assert_eq!(test_buffer[i], 255);
            }
        }

        // read some test data
        let mut buffer = TestData {
            a: 0,
            b: false,
            c: None,
        };

        unsafe { read_storage_data_into(&mut module, 10, &mut buffer).unwrap() };
        assert_eq!(original, buffer);

        let res = unsafe { read_storage_data::<TestData, T>(&mut module, 10).unwrap() };
        assert_eq!(original, res);

        // unsafe read, make sure read does not read over boundaries
        // actually incredibly unsafe: padding is not considered here
        const BUFFER_LEN: usize = 100;
        let mut buffer = [0u8; BUFFER_LEN];
        let view: &mut [u8] = &mut buffer[0..size_of::<TestData>()];
        module.read(10, view).unwrap();

        let data = unsafe {
            ((&mut buffer as *mut u8) as *mut TestData)
                .as_ref()
                .unwrap()
        };
        assert_eq!(&mut original, data);
        for i in size_of::<TestData>()..BUFFER_LEN {
            // other data should not have been changed
            assert_eq!(buffer[i], 0, "invalid data at position {}", i);
        }
    }
}
