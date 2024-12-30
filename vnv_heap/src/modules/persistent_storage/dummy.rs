use super::PersistentStorageModule;

pub struct DummyStorageModule;

impl PersistentStorageModule for DummyStorageModule {
    fn read(&mut self, _offset: usize, _dest: &mut [u8]) -> Result<(), ()> {
        panic!("not implemented")
    }

    fn get_max_size(&self) -> usize {
        panic!("not implemented")
    }

    fn write(&mut self, _offset: usize, _src: &[u8]) -> Result<(), ()> {
        panic!("not implemented")
    }
}
