use core::{
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
    cell::UnsafeCell, mem::ManuallyDrop
};
use try_lock::{Locked, TryLock};

use crate::{persist_access_point::print_persist_debug, vnv_persist_all};

pub(crate) struct SharedPersistLock<'a, T> {
    persist_queued: &'a AtomicBool,
    lock: &'a TryLock<()>,
    inner: UnsafeCell<T>,
}

impl<'a, T> SharedPersistLock<'a, T> {
    pub(crate) fn new(inner: T, persist_queued: &'a AtomicBool, lock: &'a TryLock<()>) -> Self {
        Self {
            persist_queued,
            lock,
            inner: UnsafeCell::new(inner),
        }
    }

    pub(crate) fn try_lock<'b>(&'b self) -> Option<SharedPersistGuard<'a, 'b, T>> {
        self.lock.try_lock().map(|lock| SharedPersistGuard {
            persist_queued: self.persist_queued,
            guard: ManuallyDrop::new(lock),
            obj_ref: unsafe { self.inner.get().as_mut().unwrap() },
        })
    }
}

pub(crate) struct SharedPersistGuard<'a, 'b, T> {
    persist_queued: &'a AtomicBool,
    obj_ref: &'b mut T,
    guard: ManuallyDrop<Locked<'b, ()>>,
}

impl<T> Deref for SharedPersistGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.obj_ref
    }
}

impl<T> DerefMut for SharedPersistGuard<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.obj_ref
    }
}

impl<T> Drop for SharedPersistGuard<'_, '_, T> {
    fn drop(&mut self) {
        // drop this lock guard first
        unsafe { ManuallyDrop::drop(&mut self.guard) }

        // no check if during this lock a persist was queued

        // this is free from race conditions as we require that no other threads
        // continue while vnv_persist_all is called

        if self.persist_queued.swap(false, Ordering::SeqCst) {
            print_persist_debug("persist was queued! persist now...\n");

            // persist was called during this lock call
            // call persist again, as now the lock is available again

            // as currently only one heap can be created and this heap is only active in one thread
            // its safe to assume that no other threads should be running
            // (before calling vnv_persist_all the first time, it has to be made sure that all other threads stop)
            unsafe {
                vnv_persist_all();
            }
        }
    }
}

impl<T: Clone> SharedPersistLock<'_, T> {
    pub(crate) fn try_lock_clone(&self) -> Option<Self> {
        self.try_lock().map(|guard| Self {
            persist_queued: self.persist_queued,
            lock: self.lock,
            inner: UnsafeCell::new(guard.obj_ref.clone()),
        })
    }
}
