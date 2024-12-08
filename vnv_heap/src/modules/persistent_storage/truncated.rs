use super::PersistentStorageModule;

pub struct TruncatedStorageModule<const SIZE: usize, S: PersistentStorageModule> {
    inner: S,
}

impl<const SIZE: usize, S: PersistentStorageModule> TruncatedStorageModule<SIZE, S> {
    pub fn new(storage: S) -> Self {
        assert!(storage.get_max_size() >= SIZE);

        Self {
            inner: storage
        }
    }
}

impl<const SIZE: usize, S: PersistentStorageModule> PersistentStorageModule for TruncatedStorageModule<SIZE, S> {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        debug_assert!(offset + dest.len() <= SIZE);
        self.inner.read(offset, dest)
    }

    fn get_max_size(&self) -> usize {
        SIZE
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        debug_assert!(offset + src.len() <= SIZE);
        self.inner.write(offset, src)
    }
}
