// Internal stuff used by VNVHeap's implementation

use super::PersistentStorageModule;
use crate::shared_persist_lock::SharedPersistLock;
use std::marker::PhantomData;

pub(crate) struct SharedStorageReference<'a, 'b> {
    lock: SharedPersistLock<'a, *mut dyn PersistentStorageModule>,
    _phantom_data: PhantomData<&'b ()>,
}

impl PersistentStorageModule for SharedStorageReference<'_, '_> {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        let guard = self.lock.try_lock().ok_or(())?;

        let s_ref = unsafe { guard.as_mut().unwrap() };
        s_ref.read(offset, dest)
    }

    fn get_max_size(&self) -> usize {
        // could be optimized more if you change the interface
        // however this is not necessary for now as this is only called once during initializing VNVHeap
        let guard = self.lock.try_lock().unwrap();

        let s_ref = unsafe { guard.as_mut().unwrap() };
        s_ref.get_max_size()
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        let guard = self.lock.try_lock().ok_or(())?;

        let s_ref = unsafe { guard.as_mut().unwrap() };
        s_ref.write(offset, src)
    }
}

impl<'a, 'b> SharedStorageReference<'a, 'b> {
    pub(crate) fn new(lock: SharedPersistLock<'a, *mut dyn PersistentStorageModule>) -> Self {
        Self {
            lock,
            _phantom_data: PhantomData,
        }
    }

    pub(crate) fn try_lock_clone(&self) -> Option<Self> {
        self.lock.try_lock_clone().map(|val| {
            Self {
                lock: val,
                _phantom_data: PhantomData,
            }
        })
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.lock.try_lock().is_none()
    }
}
