use super::PersistentStorageModule;

pub struct SlicedStorageModule<const SLICE_SIZE: usize, S: PersistentStorageModule> {
    inner: S,
}

impl<const SLICE_SIZE: usize, S: PersistentStorageModule> SlicedStorageModule<SLICE_SIZE, S> {
    pub fn new(storage: S) -> Self {
        Self {
            inner: storage
        }
    }
}

impl<const SLICE_SIZE: usize, S: PersistentStorageModule> PersistentStorageModule for SlicedStorageModule<SLICE_SIZE, S> {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        let mut rel_offset = 0;
        while rel_offset < dest.len() {
            let end_read = (rel_offset + SLICE_SIZE).min(dest.len());
            self.inner.read(offset + rel_offset, &mut dest[rel_offset..end_read])?;

            rel_offset += SLICE_SIZE;
        }
        
        Ok(())
    }

    fn get_max_size(&self) -> usize {
        self.inner.get_max_size()
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        let mut rel_offset = 0;
        while rel_offset < src.len() {
            let end_read = (rel_offset + SLICE_SIZE).min(src.len());
            self.inner.write(offset + rel_offset, &src[rel_offset..end_read])?;

            rel_offset += SLICE_SIZE;
        }
        Ok(())
    }
}
