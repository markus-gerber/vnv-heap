use crate::{
    benchmarks::common::multi_page::MemoryManager,
    modules::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule},
};

use super::KeyValueStoreImpl;

pub(super) struct PagedKeyValueStoreImplementation<
    'a,
    const PAGE_SIZE: usize,
    const PAGE_COUNT: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    manager: MemoryManager<'a, PAGE_SIZE, PAGE_COUNT, A, S>,
}

impl<
        'a,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > PagedKeyValueStoreImplementation<'a, PAGE_SIZE, PAGE_COUNT, A, S>
{
    pub(super) fn new(storage: &'a mut S, alloc: A, modified_page_limit: usize, pages: &'a mut [[u8; PAGE_SIZE]; PAGE_COUNT]) -> Self {
        Self {
            manager: MemoryManager::new(storage, alloc, modified_page_limit, pages),
        }
    }
}

type InternalPointer = *mut u8;

impl<
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > KeyValueStoreImpl<InternalPointer>
    for PagedKeyValueStoreImplementation<'_, PAGE_SIZE, PAGE_COUNT, A, S>
{
    fn allocate<T>(&self, data: T) -> Result<InternalPointer, ()> {
        self.manager
            .get_inner()
            .allocate(data)
            .map(|x| x as *mut u8)
    }

    fn deallocate<T>(&self, ptr: &InternalPointer) {
        self.manager.get_inner().drop_and_deallocate(*ptr as *mut T);
    }

    fn get<T: Copy>(&mut self, ptr: &InternalPointer) -> Result<T, ()> {
        // no acquire ad release here, as the pages are resident anyways
        unsafe { (*ptr as *mut T).as_ref().map(|x| *x).ok_or(()) }
    }

    fn update<T>(&mut self, ptr: &InternalPointer, data: T) -> Result<(), ()> {
        self.manager.get_inner().acquire_mut(*ptr as *mut T)?;

        unsafe {
            let ptr = *ptr as *mut T;
            *ptr = data;
        }

        self.manager.get_inner().release_mut(*ptr as *mut T);

        Ok(())
    }
    
    fn flush<T>(&mut self, ptr: &InternalPointer) -> Result<(), ()> {
        self.manager.get_inner().flush::<T>(*ptr as *mut T)
    }
}
