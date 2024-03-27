use std::{fs::{remove_file, File}, mem::ManuallyDrop, os::fd::AsRawFd, path::Path, ptr::{null_mut, NonNull}};
use libc::{c_void, mmap, msync, munmap, MAP_FAILED, MAP_SHARED, MS_SYNC, PROT_READ, PROT_WRITE};

use super::PageStorageModule;

pub struct MMapPageStorageModule {
    /// underlying file which will be mapped
    file: ManuallyDrop<File>,

    /// path of file, save for deleting file later
    file_path: &'static str,

    /// cached file size, so no `metadata` call necessary
    file_size: u64
}

impl MMapPageStorageModule {
    /// Creates as new storage module which uses mmap and msync under the hood
    pub fn new(filepath: &'static str) -> std::io::Result<MMapPageStorageModule> {
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(filepath)?;

        let file_size = file.metadata().unwrap().len();

        Ok(MMapPageStorageModule {
            file: ManuallyDrop::new(file),
            file_path: filepath,
            file_size: file_size
        })
    }
}

impl PageStorageModule for MMapPageStorageModule {
    unsafe fn map(&mut self, offset: u64, size: usize) -> Result<std::ptr::NonNull<u8>, ()> {
        let res = unsafe {
            mmap(null_mut(), size, PROT_READ | PROT_WRITE, MAP_SHARED, self.file.as_raw_fd(), offset as i64)
        };

        if res == MAP_FAILED {
            Err(())
        } else {
            if let Some(res) = NonNull::new(res as *mut u8) {
                Ok(res)
            } else {
                Err(())
            }
        }
    }

    unsafe fn unmap(&mut self, pointer: std::ptr::NonNull<u8>, size: usize) -> Result<(), ()> {
        let code = unsafe {
            munmap(pointer.as_ptr() as *mut c_void, size)
        };

        if code == 0 {
            Ok(())
        } else {
            Err(())
        }
    }

    fn persist(&mut self, pointer: std::ptr::NonNull<u8>, size: usize) -> Result<(), ()> {
        let res = unsafe {
            msync(pointer.as_ptr() as *mut c_void, size, MS_SYNC)
        };

        if res == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
    
    fn add_new_region(&mut self, size: usize) -> Result<u64, ()> {
        /*
        NOT NEEDED?
        self.file.seek(std::io::SeekFrom::End(0)).map_err(|_| ())?;

        // as normally only multiple of page sizer are added do bundled write with STD_PAGE_SIZE

        // page size of most systems
        // its no problem if actual page size is not the STD_PAGE_SIZE here
        // because write will success any way (just slower tho)
        const STD_PAGE_SIZE: usize = 4096;
        const WRITE_ARR: [u8; STD_PAGE_SIZE] = [0; STD_PAGE_SIZE];

        let mut write_counter = 0;
        while write_counter + STD_PAGE_SIZE < size {
            self.file.write_all(&WRITE_ARR).map_err(|_| ())?;
            write_counter += STD_PAGE_SIZE;
        }

        // do single writes now for the rest
        let rest_count = size - write_counter;
        if rest_count != 0 {
            println!("[DEBUG] MMapPageStorageModule: add_new_region needs to do {} unbundled writes", rest_count);
            for _ in 0..(size - write_counter) {
                self.file.write_all(&[0]).map_err(|_| ())?;
            }
        }

        self.file.flush().unwrap();

        let prev_size = self.file_size;
        self.file_size += size;

        Ok(prev_size)
        */

        let new_size = self.file_size + (size as u64);
        self.file.set_len(new_size as u64).map_err(|_| ())?;

        // memorize previous file size for returning offset pointer
        let prev_size = self.file_size;
        
        // just increase cached size as file was successfully resized
        self.file_size = new_size;

        Ok(prev_size)     
    }
}

impl Drop for MMapPageStorageModule {
    fn drop(&mut self) {
        // drop and close file before removing
        // note that after this call, file should never be accessed again...
        unsafe {
            ManuallyDrop::drop(&mut self.file);
        }

        if Path::new(self.file_path).exists() {
            let _ = remove_file(self.file_path);
        }
    }
}

#[cfg(test)]
mod test {
    use std::ptr::slice_from_raw_parts_mut;
    use crate::modules::page_storage::PageStorageModule;
    use super::MMapPageStorageModule;

    /// test if sync saves all data, mmap restores it after data was unmaped
    #[test]
    fn test_persist_single_page() {
        const PAGE_SIZE: usize = 4096;

        // this is our expected data
        let mut source_slice = [0u8; PAGE_SIZE];
        for i in 0..PAGE_SIZE {
            source_slice[i] = (i + 1) as u8;
        }

        let mut storage = MMapPageStorageModule::new("mmap_test.tmp").unwrap();
        assert_eq!(storage.file.metadata().unwrap().len(), storage.file_size, "cached file size does not match the actual file size!");
        assert_eq!(storage.file_size, 0, "size of file should be zero first (it should be fresh file)!");

        {
            let ptr = unsafe { storage.map_new_region(PAGE_SIZE) }.unwrap();
            assert_eq!(storage.file.metadata().unwrap().len(), storage.file_size, "cached file size does not match the actual file size!");
            assert_eq!(storage.file_size, PAGE_SIZE as u64, "size of file should be one PAGE_SIZE!");

            let slice = unsafe { slice_from_raw_parts_mut(ptr.as_ptr(), PAGE_SIZE).as_mut().unwrap() };
            
            for i in 0..PAGE_SIZE {
                slice[i] = source_slice[i];
            }

            storage.persist(ptr, PAGE_SIZE).unwrap();

            unsafe { storage.unmap(ptr, PAGE_SIZE).unwrap(); }
        }

        {
            let ptr = unsafe { storage.map(0, PAGE_SIZE) }.unwrap();
            let slice = unsafe { slice_from_raw_parts_mut(ptr.as_ptr(), PAGE_SIZE).as_mut().unwrap() };
            
            for i in 0..PAGE_SIZE {
                assert!(slice[i] == source_slice[i]);
            }

            unsafe { storage.unmap(ptr, PAGE_SIZE).unwrap(); }
        }
    }

}