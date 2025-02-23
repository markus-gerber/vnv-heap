use std::{
    cell::{RefCell, RefMut},
    ops::{Deref, DerefMut},
};

use super::{memory_manager::MemoryManagerInner, AllocatorModule, PersistentStorageModule};

pub(crate) struct Object<
    'a,
    'b,
    T,
    const PAGE_SIZE: usize,
    const PAGE_COUNT: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    ptr: *mut T,
    inner: &'a RefCell<MemoryManagerInner<'b, PAGE_SIZE, PAGE_COUNT, A, S>>,
}

impl<
        'a,
        'b,
        T,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > Object<'a, 'b, T, PAGE_SIZE, PAGE_COUNT, A, S>
{
    pub(crate) fn new(
        ptr: *mut T,
        inner: &'a RefCell<MemoryManagerInner<'b, PAGE_SIZE, PAGE_COUNT, A, S>>,
    ) -> Self {
        Self { ptr, inner }
    }

    #[allow(unused)]
    pub(crate) fn get_inner(
        &self,
    ) -> RefMut<'_, MemoryManagerInner<'b, PAGE_SIZE, PAGE_COUNT, A, S>> {
        self.inner.borrow_mut()
    }

    #[allow(unused)]
    pub(crate) fn get_ref<'c>(
        &'c self,
    ) -> Result<&T, ()> {
        // no additional work as pages are required to be resident at all time for this implementation

        Ok(unsafe { self.ptr.as_ref().unwrap() })
    }

    #[allow(unused)]
    pub(crate) fn get_mut<'c>(
        &'c self,
    ) -> Result<ObjectMutRef<'a, 'b, 'c, T, PAGE_SIZE, PAGE_COUNT, A, S>, ()> {
        {
            let mut inner = self.inner.borrow_mut();
            inner.acquire_mut(self.ptr)?;
        }

        Ok(unsafe { ObjectMutRef::new(&self.inner, self.ptr.as_mut().unwrap()) })
    }
}

impl<
        'a,
        'b,
        T,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > Drop for Object<'_, '_, T, PAGE_SIZE, PAGE_COUNT, A, S>
{
    fn drop(&mut self) {
        self.inner.borrow_mut().drop_and_deallocate(self.ptr);
    }
}


pub struct ObjectMutRef<
    'a,
    'b,
    'c,
    T: Sized,
    const PAGE_SIZE: usize,
    const PAGE_COUNT: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    inner: &'a RefCell<MemoryManagerInner<'b, PAGE_SIZE, PAGE_COUNT, A, S>>,
    data_ref: &'c mut T,
}

impl<
        'a,
        'b,
        'c,
        T: Sized,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > ObjectMutRef<'a, 'b, 'c, T, PAGE_SIZE, PAGE_COUNT, A, S>
{
    pub(crate) unsafe fn new(
        inner: &'a RefCell<MemoryManagerInner<'b, PAGE_SIZE, PAGE_COUNT, A, S>>,
        data_ref: &'c mut T,
    ) -> Self {
        ObjectMutRef { inner, data_ref }
    }
}

impl<
        T: Sized,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > Deref for ObjectMutRef<'_, '_, '_, T, PAGE_SIZE, PAGE_COUNT, A, S>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<
        T: Sized,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > DerefMut for ObjectMutRef<'_, '_, '_, T, PAGE_SIZE, PAGE_COUNT, A, S>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_ref
    }
}

impl<
        T: Sized,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > Drop for ObjectMutRef<'_, '_, '_, T, PAGE_SIZE, PAGE_COUNT, A, S>
{
    fn drop(&mut self) {
        self.inner.borrow_mut().release_mut::<T>(self.data_ref);
    }
}
