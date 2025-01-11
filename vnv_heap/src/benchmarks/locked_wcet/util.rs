use core::panic;
use std::{marker::PhantomData, mem::{transmute, ManuallyDrop}, ops::{Deref, DerefMut}, sync::{atomic::{AtomicBool, AtomicPtr, Ordering}, Mutex}};

use try_lock::TryLock;

use crate::{benchmarks::GetCurrentTicks, shared_persist_lock::{SharedPersistGuard, SharedPersistLock}};

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
    pub(super) fn persist_if_not_empty(&self, start_time: u32, get_curr_ticks: GetCurrentTicks) -> u32 {
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

            return get_curr_ticks() - start_time;
        }

        panic!("should not happen");
    }

}

pub(super) static GET_CURR_TICKS: Mutex<Option<GetCurrentTicks>> = Mutex::new(None);

pub(super) static mut BENCHMARKABLE_PERSIST_ACCESS_POINT: BenchmarkablePersistAccessPoint = BenchmarkablePersistAccessPoint::empty();
pub(super) static BENCHMARKABLE_HEAP_LOCK: TryLock<()> = TryLock::new(());
pub(super) static BENCHMARKABLE_STORAGE_LOCK: TryLock<()> = TryLock::new(());
pub(super) static BENCHMARKABLE_PERSIST_QUEUED: AtomicBool = AtomicBool::new(false);


pub(super) fn benchmarkable_vnv_persist_all(start_time: u32, get_curr_ticks: GetCurrentTicks) -> u32 {
    unsafe { BENCHMARKABLE_PERSIST_ACCESS_POINT.persist_if_not_empty(start_time, get_curr_ticks) }
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
    pub(super) fn read_benchmarked(&mut self, offset: usize, dest: &mut [u8]) -> Result<u32, ()> {
        let guard = self.lock.try_lock().ok_or(())?;

        let s_ref = unsafe { guard.as_mut().unwrap() };
        s_ref.read(offset, dest)?;
        Ok(guard.measured_drop())
    }
    pub(super) fn write_benchmarked(&mut self, offset: usize, src: &[u8]) -> Result<u32, ()> {
        let guard = self.lock.try_lock().ok_or(())?;

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
    
    pub(super) fn try_lock<'b>(&'b self) -> Option<BenchmarkableSharedPersistGuard<'a, 'b, T>> {
        let get_curr_ticks = { GET_CURR_TICKS.lock().unwrap().unwrap().clone() };
        let start_time = get_curr_ticks();
        if let Some(res) = self.inner.try_lock() {
            Some(BenchmarkableSharedPersistGuard { inner: ManuallyDrop::new(res), start_time, get_curr_ticks })
        } else {
            None
        }
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


pub(super) struct BenchmarkableSharedPersistGuard<'a, 'b, T> {
    inner: ManuallyDrop<SharedPersistGuard<'a, 'b, T>>,
    start_time: u32,
    get_curr_ticks: GetCurrentTicks
}


impl<T> BenchmarkableSharedPersistGuard<'_, '_, T> {
    pub(super) fn measured_drop(mut self) -> u32 {
        // do the same steps as in SharedPersistGuard::drop
        unsafe { ManuallyDrop::drop(&mut self.inner.guard) }

        // we dont care about the result here as we just want to measure the latency
        if self.inner.persist_queued.swap(false, Ordering::SeqCst) {
            let time = benchmarkable_vnv_persist_all(self.start_time, self.get_curr_ticks);

            time
        } else {
            panic!("persist queued was not set before!");
        }
    }
}

impl<T> Drop for BenchmarkableSharedPersistGuard<'_, '_, T> {
    fn drop(&mut self) {
        // do not do any measurements or persists
        unsafe { ManuallyDrop::drop(&mut self.inner.guard) }
    }
}


impl<T> Deref for BenchmarkableSharedPersistGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.obj_ref
    }
}

impl<T> DerefMut for BenchmarkableSharedPersistGuard<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.obj_ref
    }
}
