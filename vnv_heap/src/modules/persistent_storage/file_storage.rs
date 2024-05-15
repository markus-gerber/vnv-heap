use std::{
    fs::{remove_file, File},
    io::{Read, Seek, Write},
    mem::ManuallyDrop,
    path::Path,
};

use super::PersistentStorageModule;

pub struct FilePersistentStorageModule {
    /// underlying file which will be mapped
    file: ManuallyDrop<File>,

    /// path of file, save for deleting file later
    file_path: String,

    /// cached file size, so no `metadata` call necessary
    file_size: usize,
}

impl FilePersistentStorageModule {
    /// Creates as new storage module which uses mmap and msync under the hood
    pub fn new(filepath: String, size: usize) -> std::io::Result<Self> {
        let file = File::options()
            .read(true)
            .write(true)
            .truncate(true)
            .create(true)
            .open(filepath.clone())?;

        file.set_len(size as u64)?;

        Ok(Self {
            file: ManuallyDrop::new(file),
            file_path: filepath,
            file_size: size,
        })
    }
}

impl PersistentStorageModule for FilePersistentStorageModule {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        debug_assert!(offset + dest.len() <= self.file_size);

        self.file
            .seek(std::io::SeekFrom::Start(offset as u64))
            .map_err(|_| ())?;
        self.file.read_exact(dest).map_err(|_| ())?;

        Ok(())
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        debug_assert!(offset + src.len() <= self.file_size);

        self.file
            .seek(std::io::SeekFrom::Start(offset as u64))
            .map_err(|_| ())?;
        self.file.write_all(src).map_err(|_| ())?;

        Ok(())
    }

    fn get_max_size(&self) -> usize {
        self.file_size
    }
}

impl Drop for FilePersistentStorageModule {
    fn drop(&mut self) {
        // drop and close file before removing
        // note that after this call, file should never be accessed again...
        unsafe {
            ManuallyDrop::drop(&mut self.file);
        }

        if Path::new(self.file_path.as_str()).exists() {
            let _ = remove_file(self.file_path.as_str());
        }
    }
}

#[cfg(test)]
mod test {
    use crate::modules::persistent_storage::test::{
        PERSISTENT_STORAGE_CUSTOM_TYPE_TEST_SIZE, PERSISTENT_STORAGE_NORMAL_TEST_SIZE,
    };

    use super::super::test::{test_persistent_storage_custom_type, test_persistent_storage_normal};
    use super::FilePersistentStorageModule;

    #[test]
    fn test_file_storage_module_normal() {
        let storage = FilePersistentStorageModule::new(
            "/tmp/test_file_storage_module_normal.tmp".into(),
            PERSISTENT_STORAGE_NORMAL_TEST_SIZE,
        )
        .unwrap();
        test_persistent_storage_normal(storage);
    }

    #[test]
    fn test_file_storage_module_custom_types() {
        let storage = FilePersistentStorageModule::new(
            "/tmp/test_file_storage_module_custom_types.tmp".into(),
            PERSISTENT_STORAGE_CUSTOM_TYPE_TEST_SIZE,
        )
        .unwrap();
        test_persistent_storage_custom_type(storage);
    }
}
