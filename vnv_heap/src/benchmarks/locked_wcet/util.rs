use core::panic;
use std::{marker::PhantomData, mem::{transmute, ManuallyDrop}, ops::{Deref, DerefMut}, sync::atomic::{AtomicBool, Ordering}};

use try_lock::TryLock;

use crate::{benchmarks::Timer, shared_persist_lock::{SharedPersistGuard, SharedPersistLock}};

use super::microbenchmarks::PersistentStorageModule;

pub(super) struct BenchmarkablePersistAccessPointInner {
    heap_lock: &'static TryLock<()>,
    storage: BenchmarkableSharedStorageReference<'static, 'static>
}

pub(super) struct BenchmarkablePersistAccessPoint {
    inner: TryLock<Option<BenchmarkablePersistAccessPointInner>>,
}

impl BenchmarkablePersistAccessPoint {
    pub(super) const fn empty() -> Self {
        Self {
            inner: TryLock::new(None),
        }
    }

    pub(super) unsafe fn set<'a, 'b>(
        &mut self,
        storage: BenchmarkableSharedStorageReference<'a, 'b>,
    ) -> Result<(), ()> {
        let mut lock_guard = self.inner.try_lock().ok_or(())?;

        if lock_guard.is_some() {
            return Err(());
        }

        *lock_guard = Some(BenchmarkablePersistAccessPointInner {
            storage: transmute(storage),
            heap_lock: &BENCHMARKABLE_HEAP_LOCK,
        });

        drop(lock_guard);

        Ok(())
    }

    pub(super) fn unset(&mut self) -> Result<(), ()> {
        let mut lock_guard = self.inner.try_lock().ok_or(())?;
        *lock_guard = None;

        Ok(())
    }


}

impl BenchmarkablePersistAccessPoint {
    pub(super) fn persist_if_not_empty<F: FnOnce() -> u32>(&self, measurement_stop: F) -> u32 {
        let mut lock_guard = match self.inner.try_lock() {
            Some(guard) => guard,
            None => {
                // If this is locked here it means that set or unset is called right now
                // as in both cases the vnv heap is not fully initialized yet or is currently being dropped
                // we don't need to save it

                // However, this would need to change if you want to have multiple VNVHeaps that should be persisted
                panic!("should not happen");
            }
        };

        if let Some(inner) = lock_guard.as_mut() {
            // ###### TRY TO GET ALL NECESSARY LOCKS ######

            {
                // there wont be any race conditions here as its guaranteed that no other threads
                // run during this handler
                if inner.heap_lock.try_lock().is_none() || inner.storage.is_locked() {
                    panic!("should not happen");
                }
            }

            return measurement_stop();
        }

        panic!("should not happen");
    }

}

pub(super) static mut BENCHMARKABLE_PERSIST_ACCESS_POINT: BenchmarkablePersistAccessPoint = BenchmarkablePersistAccessPoint::empty();
pub(super) static BENCHMARKABLE_HEAP_LOCK: TryLock<()> = TryLock::new(());
pub(super) static BENCHMARKABLE_STORAGE_LOCK: TryLock<()> = TryLock::new(());
pub(super) static BENCHMARKABLE_PERSIST_QUEUED: AtomicBool = AtomicBool::new(false);


pub(super) fn benchmarkable_vnv_persist_all<F: FnOnce() -> u32>(measurement_stop: F) -> u32 {
    unsafe { BENCHMARKABLE_PERSIST_ACCESS_POINT.persist_if_not_empty(measurement_stop) }
}

pub(super) struct BenchmarkableSharedStorageReference<'a, 'b> {
    lock: BenchmarkableSharedPersistLock<'a, *mut dyn PersistentStorageModule>,
    _phantom_data: PhantomData<&'b ()>,
}

impl PersistentStorageModule for BenchmarkableSharedStorageReference<'_, '_> {
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

impl BenchmarkableSharedStorageReference<'_, '_> {
    pub(super) fn read_benchmarked<TIMER: Timer>(&mut self, offset: usize, dest: &mut [u8]) -> Result<u32, ()> {
        let guard = self.lock.try_lock_measured::<TIMER>().ok_or(())?;

        let s_ref = unsafe { guard.as_mut().unwrap() };
        s_ref.read(offset, dest)?;
        Ok(guard.measured_drop())
    }
    pub(super) fn write_benchmarked<TIMER: Timer>(&mut self, offset: usize, src: &[u8]) -> Result<u32, ()> {
        let guard = self.lock.try_lock_measured::<TIMER>().ok_or(())?;

        let s_ref = unsafe { guard.as_mut().unwrap() };
        s_ref.write(offset, src)?;
        Ok(guard.measured_drop())
    }
}

impl<'a, 'b> BenchmarkableSharedStorageReference<'a, 'b> {
    pub(super) fn new(lock: BenchmarkableSharedPersistLock<'a, *mut dyn PersistentStorageModule>) -> Self {
        Self {
            lock,
            _phantom_data: PhantomData,
        }
    }

    pub(super) fn is_locked(&self) -> bool {
        self.lock.try_lock().is_none()
    }

    pub(crate) fn try_lock_clone(&self) -> Option<Self> {
        self.lock.try_lock_clone().map(|val| {
            Self {
                lock: val,
                _phantom_data: PhantomData,
            }
        })
    }

}

pub(super) struct BenchmarkableSharedPersistLock<'a, T> {
    inner: SharedPersistLock<'a, T>,
}


impl<'a, T> BenchmarkableSharedPersistLock<'a, T> {
    pub(super) fn new(inner: T, persist_queued: &'a AtomicBool, lock: &'a TryLock<()>) -> Self {
        Self {
            inner: SharedPersistLock::new(inner, persist_queued, lock)
        }
    }
    
    pub(super) fn try_lock_measured<'b, TIMER: Timer>(&'b self) -> Option<BenchmarkableSharedPersistGuard<'a, 'b, T, TIMER>> {
        let timer = TIMER::start();
        if let Some(res) = self.inner.try_lock() {
            Some(BenchmarkableSharedPersistGuard { inner: ManuallyDrop::new(res), timer: Some(timer) })
        } else {
            None
        }
    }

    
    pub(super) fn try_lock<'b>(&'b self) -> Option<SharedPersistGuard<'a, 'b, T>> {
        self.inner.try_lock()
    }

}

impl<T: Clone> BenchmarkableSharedPersistLock<'_, T> {
    pub(crate) fn try_lock_clone(&self) -> Option<Self> {
        if let Some(inner) = self.inner.try_lock_clone() {
            Some(BenchmarkableSharedPersistLock {
                inner
            })
        } else {
            None
        }

    }
}


pub(super) struct BenchmarkableSharedPersistGuard<'a, 'b, T, TIMER: Timer> {
    inner: ManuallyDrop<SharedPersistGuard<'a, 'b, T>>,
    timer: Option<TIMER>,
}


impl<T, TIMER: Timer> BenchmarkableSharedPersistGuard<'_, '_, T, TIMER> {
    pub(super) fn measured_drop(mut self) -> u32 {
        // do the same steps as in SharedPersistGuard::drop
        unsafe { ManuallyDrop::drop(&mut self.inner.guard) }

        // we dont care about the result here as we just want to measure the latency
        if self.inner.persist_queued.swap(false, Ordering::SeqCst) {
            let timer = self.timer.take().unwrap();
            let time = benchmarkable_vnv_persist_all(move || {
                timer.stop()
            });

            time
        } else {
            panic!("persist queued was not set before!");
        }
    }
}

impl<T, TIMER: Timer> Drop for BenchmarkableSharedPersistGuard<'_, '_, T, TIMER> {
    fn drop(&mut self) {
        // do not do any measurements or persists
        unsafe { ManuallyDrop::drop(&mut self.inner.guard) }
    }
}


impl<T, TIMER: Timer> Deref for BenchmarkableSharedPersistGuard<'_, '_, T, TIMER> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.obj_ref
    }
}

impl<T, TIMER: Timer> DerefMut for BenchmarkableSharedPersistGuard<'_, '_, T, TIMER> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.obj_ref
    }
}
