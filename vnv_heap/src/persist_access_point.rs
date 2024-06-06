use core::{ptr::null_mut, mem::{transmute, MaybeUninit}};
use try_lock::TryLock;

use crate::{modules::persistent_storage::SharedStorageReference, resident_object_manager::SharedMetadataBackupPtr};


/// An object containing all necessary data 
pub(crate) struct PersistAccessPoint {
    inner: TryLock<PersistAccessPointInner>
}

impl PersistAccessPoint {
    pub(crate) const fn empty() -> Self {
        Self {
            inner: TryLock::new(PersistAccessPointInner::empty())
        }
    }

    /// ### Safety
    /// 
    /// You need to make sure that the pointers of `dirty_list` and `storage_ref` remain valid until `unset` is called
    /// 
    /// If not, this will result in **Undefined Behavior**!
    pub(crate) unsafe fn set(&mut self, dirty_list: *mut u8, backup_list: SharedMetadataBackupPtr, storage_ref: SharedStorageReference) -> Result<(), ()> {
        // should not fail as there should currently only be one heap which is setting this
        // it is also not possible for unset to be calling it at the same time as set and
        // persist_if_not_empty is just executed in an interrupt handler which is required to be the only thread running at that time

        let mut lock_guard = self.inner.try_lock().ok_or(())?;
        lock_guard.set(dirty_list, backup_list, storage_ref)
    }

    pub(crate) fn unset(&mut self) -> Result<(), ()> {
        // should not fail as there should currently only be one heap which is setting this
        // it is also not possible for unset to be calling it at the same time as set and
        // persist_if_not_empty is just executed in an interrupt handler which is required to be the only thread running at that time
        let mut lock_guard = self.inner.try_lock().ok_or(())?;
        lock_guard.unset();

        Ok(())
    }

    pub(crate) fn persist_if_not_empty(&self) {
        let lock_guard = match self.inner.try_lock() {
            Some(guard) => guard,
            None => {
                // If this is locked here it means that set or unset is called right now
                // as in both cases the vnv heap is not fully initialized yet or is currently being dropped
                // we don't need to save it

                // However, this would need to change if you want to have multiple VNVHeaps that should be persisted
                return;
            }
        };

        lock_guard.persist_if_not_empty();
    }
}


struct PersistAccessPointInner {
    dirty_list: *mut u8,

    shared_metadata_backup_list_ptr: MaybeUninit<SharedMetadataBackupPtr<'static>>,

    /// save some space by using MaybeUninit instead of Option
    /// if `dirty_list != null` this is already initialized
    storage: MaybeUninit<SharedStorageReference<'static, 'static>>
}

impl PersistAccessPointInner {
    const fn empty() -> Self {
        Self {
            dirty_list: null_mut(),
            shared_metadata_backup_list_ptr: MaybeUninit::uninit(),
            storage: MaybeUninit::uninit()
        }
    }

    pub(crate) unsafe fn set(&mut self, dirty_list: *mut u8, backup_list: SharedMetadataBackupPtr, storage_ref: SharedStorageReference) -> Result<(), ()> {
        if !self.dirty_list.is_null() {
            return Err(());
        }

        self.dirty_list = dirty_list;

        // extending live-times to 'static
        // this is save (at least here) as the caller is required to call unset once
        // before the original live-times become invalid
        let backup_list = transmute(backup_list);

        // write to storage without running drop on uninitialized data
        self.shared_metadata_backup_list_ptr.write(backup_list);

        // now the same for storage_ref...

        // extending live-times to 'static
        // this is save (at least here) as the caller is required to call unset once
        // before the original live-times become invalid
        let storage_ref = transmute(storage_ref);

        // write to storage without running drop on uninitialized data
        self.storage.write(storage_ref);

        Ok(())
    }

    pub(crate) fn unset(&mut self) {
        if !self.dirty_list.is_null() {
            unsafe { self.storage.assume_init_drop() };
            unsafe { self.shared_metadata_backup_list_ptr.assume_init_drop() };
            self.dirty_list = null_mut();
        }
    }

    pub(crate) fn persist_if_not_empty(&self) {
        if self.dirty_list.is_null() {
            return;
        }

        // TODO...
    }

}

unsafe impl Send for PersistAccessPointInner {}