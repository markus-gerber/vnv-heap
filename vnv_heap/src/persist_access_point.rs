use core::{
    mem::transmute,
    sync::atomic::{AtomicBool, Ordering},
};
use std::sync::atomic::AtomicPtr;
use try_lock::TryLock;

use crate::{
    modules::{allocator::AllocatorModule, persistent_storage::SharedStorageReference},
    resident_object_manager::{
        persist, resident_list::SharedResidentListRef, restore,
    },
};

/// An object containing all necessary data
/// 
/// We need this as the VNVHeap object can be moved (if no objects).
/// This would would break things, as we need to store a pointer the the necessary data once the VNVHeap is created
/// so that `vnv_persist_all()` can access the needed data
pub(crate) struct PersistAccessPoint {
    inner: TryLock<Option<PersistAccessPointInner>>,
}

impl PersistAccessPoint {
    pub(crate) const fn empty() -> Self {
        Self {
            inner: TryLock::new(None),
        }
    }

    /// ### Safety
    ///
    /// You need to make sure that the pointers of `resident_list` and `storage_ref` remain valid until `unset` is called
    ///
    /// If not, this will result in **Undefined Behavior**!
    pub(crate) unsafe fn set<'a, 'b>(
        &mut self,
        base_ptr: *mut u8,
        buf_size: usize,
        resident_list: SharedResidentListRef<'a>,
        storage: SharedStorageReference<'a, 'b>,
        handler: fn(*mut u8, usize) -> (),
        heap_lock: &TryLock<()>,
        persist_queued: &AtomicBool,
        heap: *mut dyn AllocatorModule,
    ) -> Result<(), ()> {
        // should not fail as there should currently only be one heap which is setting this
        // it is also not possible for unset to be calling it at the same time as set and
        // persist_if_not_empty is just executed in an interrupt handler which is required to be the only thread running at that time

        let mut lock_guard = self.inner.try_lock().ok_or(())?;

        if lock_guard.is_some() {
            // already in use
            return Err(());
        }

        *lock_guard = Some(PersistAccessPointInner {
            resident_buf_base_ptr: base_ptr,
            resident_buf_size: buf_size,
            // change the lifetime of these values
            resident_list: transmute(resident_list),
            storage: transmute(storage),
            heap_lock: transmute(heap_lock),
            persist_queued: transmute(persist_queued),
            handler,
            heap,
        });

        drop(lock_guard);

        Ok(())
    }

    pub(crate) fn unset(&mut self) -> Result<(), ()> {
        // should not fail as there should currently only be one heap which is setting this
        // it is also not possible for unset to be calling it at the same time as set and
        // persist_if_not_empty is just executed in an interrupt handler which is required to be the only thread running at that time
        let mut lock_guard = self.inner.try_lock().ok_or(())?;
        *lock_guard = None;

        Ok(())
    }

    pub(crate) fn persist_if_not_empty(&self) {
        let mut lock_guard = match self.inner.try_lock() {
            Some(guard) => guard,
            None => {
                // If this is locked here it means that set or unset is called right now
                // as in both cases the vnv heap is not fully initialized yet or is currently being dropped
                // we don't need to save it

                // However, this would need to change if you want to have multiple VNVHeaps that should be persisted
                return;
            }
        };

        if let Some(inner) = lock_guard.as_mut() {
            print_persist_debug("persist was triggered\n");

            // ###### TRY TO GET ALL NECESSARY LOCKS ######

            {
                // there wont be any race conditions here as its guaranteed that no other threads
                // run during this handler
                if inner.heap_lock.try_lock().is_none() || inner.storage.is_locked() {
                    print_persist_debug("cannot acquire lock. persist queued...\n");

                    inner.persist_queued.store(true, Ordering::SeqCst);
                    return;
                }
            }

            #[cfg(debug_assertions)]
            let metadata_backup = collect_metadata(&inner.resident_list);

            // ###### START PERSISTING STATE ######
            persist(&inner.resident_list, &mut inner.storage);

            // ###### FINISHED PERSISTING STATE: EXECUTING HANDLER NOW ######
            (inner.handler)(inner.resident_buf_base_ptr, inner.resident_buf_size);

            // ###### HANDLER RETURNED: RESTORING STATE NOW ######
            restore(
                &mut inner.storage,

                // this is safe as we could access the heap_lock
                unsafe { inner.heap.as_mut().unwrap() },
                inner.resident_buf_base_ptr,
                inner.resident_buf_size
            );

            #[cfg(debug_assertions)]
            check_metadata(&inner.resident_list, metadata_backup);
            
            print_persist_debug("restore finished\n");
        }
    }
}


#[cfg(debug_assertions)]
use crate::resident_object_manager::resident_object_metadata::ResidentObjectMetadata;

#[cfg(debug_assertions)]
fn collect_metadata(resident_list: &SharedResidentListRef<'static>) -> Vec<(*const ResidentObjectMetadata, ResidentObjectMetadata)> {
    
    let mut res: Vec<(*const ResidentObjectMetadata, ResidentObjectMetadata)> = vec![];

    let mut iter = resident_list.iter();
    while let Some(item) = iter.next() {
        let copy = ResidentObjectMetadata {
            inner: item.inner.clone(),
            next_resident_object: AtomicPtr::new(item.next_resident_object.load(Ordering::SeqCst))
        };

        res.push((item, copy));
    }

    return res;
}

#[cfg(debug_assertions)]
fn check_metadata(resident_list: &SharedResidentListRef<'static>, list: Vec<(*const ResidentObjectMetadata, ResidentObjectMetadata)>) {

    let mut iter = resident_list.iter();
    let mut vec_iter = list.iter();

    loop {
        let item1 = iter.next();
        let item2 = vec_iter.next();

        if let (Some(item1), Some(item2)) = (item1, item2) {
            assert_eq!((item1 as *const ResidentObjectMetadata), item2.0);
            assert_eq!(item1.next_resident_object.load(Ordering::SeqCst), item2.1.next_resident_object.load(Ordering::SeqCst));
            assert!(item1.inner.dirty_status == item2.1.inner.dirty_status);
            assert_eq!(item1.inner.layout, item2.1.inner.layout);
            assert_eq!(item1.inner.offset, item2.1.inner.offset);

        } else {
            assert_eq!(item1.is_none(), item2.is_none(), "The lists have different sizes!");
            break;
        }
    }
}


#[cfg(all(not(feature = "persist_debug_prints"), not(feature = "persist_debug_unsafe_prints")))]
pub(crate) fn print_persist_debug(_text: &str) {
    // do nothing
}

#[cfg(feature = "persist_debug_prints")]
pub(crate) fn print_persist_debug(text: &str) {
    // this could be called from a signal handler, so do not use print
    unsafe { libc::write(libc::STDOUT_FILENO, text.as_ptr() as *const libc::c_void, text.len()) };
}

#[cfg(feature = "persist_debug_unsafe_prints")]
pub(crate) fn print_persist_debug(text: &str) {
    // using print in a signal handler is not safe
    print!("{}", text);
}

struct PersistAccessPointInner {
    resident_buf_base_ptr: *mut u8,
    resident_buf_size: usize,
    resident_list: SharedResidentListRef<'static>,
    storage: SharedStorageReference<'static, 'static>,
    handler: fn(*mut u8, usize) -> (),
    heap_lock: &'static TryLock<()>,
    persist_queued: &'static AtomicBool,
    heap: *mut dyn AllocatorModule,
}

unsafe impl Send for PersistAccessPoint {}
unsafe impl Sync for PersistAccessPoint {}
