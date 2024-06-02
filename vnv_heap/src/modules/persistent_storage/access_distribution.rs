// Internal stuff used by VNVHeap's implementation

use core::sync::atomic::{AtomicBool, Ordering};

use try_lock::TryLock;

use crate::vnv_heap::vnv_persist_all;

use super::PersistentStorageModule;

pub(crate) struct SharedStorageAccessControl<'a> {
    persist_queued: AtomicBool,
    storage: TryLock<&'a mut dyn PersistentStorageModule>
}

impl<'a> SharedStorageAccessControl<'a> {
    pub(crate) fn new<S: PersistentStorageModule>(storage: &'a mut S) -> Self {
        Self {
            persist_queued: AtomicBool::new(false),
            storage: TryLock::new(storage)
        }
    }
}

pub(crate) struct SharedStorageReference<'a, 'b> {
    access_control: &'a SharedStorageAccessControl<'b>
}

// define critical section in which storage can be accessed freely
macro_rules! storage_reference_critical_section_inner {
    ($self: ident, $guard_name: ident, $code: block, $val_qual_macro: ident) => {
        {
            if let Some($val_qual_macro!($guard_name)) = $self.access_control.storage.try_lock() {
                let res = {
                    $code
                };

                drop($guard_name);

                // this is free from race conditions as we require that no other threads 
                // continue while vnv_persist_all is called

                if $self.access_control.persist_queued.swap(false, Ordering::SeqCst) {
                    // persist was called during this read call
                    // call persist again, as now the storage module is available again

                    // as currently only one heap can be created and this heap is only active in one thread
                    // its safe to assume that no other threads should be running
                    // (before calling vnv_persist_all the first time, it has to be made sure that all other threads stop)
                    unsafe { vnv_persist_all(); }
                }

                Ok(res)
            } else {
                Err(())
            }
        }
    };
}

// define critical section in which storage can be accessed freely
// use this so we can define mut and immutable guards at the same time
macro_rules! storage_reference_critical_section {
    ($self: ident, $guard_name: ident, $code: block) => {
        {
            macro_rules! var_qualifier {
                ($v:ident) => { $v }
            }
            storage_reference_critical_section_inner!($self, $guard_name, $code, var_qualifier)
        }
    };
    ($self: ident, mut $guard_name: ident, $code: block) => {
        {
            macro_rules! var_qualifier {
                ($v:ident) => { mut $v }
            }
            storage_reference_critical_section_inner!($self, $guard_name, $code, var_qualifier)
        }
    };
}

impl PersistentStorageModule for SharedStorageReference<'_, '_> {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        storage_reference_critical_section!(self, mut storage_guard, {
            storage_guard.read(offset, dest);
        })
    }

    fn get_max_size(&self) -> usize {
        // could be optimized more if you change the interface
        // however this is not necessary for now as this is only called once during initializing VNVHeap
        storage_reference_critical_section!(self, storage_guard, {
            storage_guard.get_max_size()
        }).unwrap()
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        storage_reference_critical_section!(self, mut storage_guard, {
            storage_guard.write(offset, src);
        })
    }
}

impl<'a, 'b> SharedStorageReference<'a, 'b> {
    pub(crate) fn new(access_control: &'a SharedStorageAccessControl<'b>) -> Self {
        Self {
            access_control
        }
    }
}

impl Clone for SharedStorageReference<'_, '_> {
    fn clone(&self) -> Self {
        Self { access_control: self.access_control.clone() }
    }
}
