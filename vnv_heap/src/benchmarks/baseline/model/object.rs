use std::{
    cell::{RefCell, RefMut},
    ops::{Deref, DerefMut},
};

use super::{memory_manager::MemoryManagerInner, AllocatorModule, PersistentStorageModule};

pub(crate) struct Object<
    'a,
    'b,
    T,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    bucket_id: usize,
    ptr: *mut T,
    inner: &'a RefCell<MemoryManagerInner<'b, BUCKET_SIZE, A, S>>,
}

impl<'a, 'b, T, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule>
    Object<'a, 'b, T, BUCKET_SIZE, A, S>
{
    pub(crate) fn new(
        bucket_id: usize,
        ptr: *mut T,
        inner: &'a RefCell<MemoryManagerInner<'b, BUCKET_SIZE, A, S>>,
    ) -> Self {
        Self {
            bucket_id,
            ptr,
            inner,
        }
    }

    pub(crate) fn get_inner(&self) -> RefMut<'_, MemoryManagerInner<'b, BUCKET_SIZE, A, S>> {
        self.inner.borrow_mut()
    }

    pub(crate) fn get_ref<'c>(&'c self) -> Result<ObjectRef<'a, 'b, 'c, T, BUCKET_SIZE, A, S>, ()> {
        {
            let mut inner = self.inner.borrow_mut();
            inner.require_resident(self.bucket_id)?;
            inner.acquire_ref();
        }

        Ok(unsafe { ObjectRef::new(&self.inner, self.ptr.as_ref().unwrap()) })
    }

    pub(crate) fn get_mut<'c>(
        &'c self,
    ) -> Result<ObjectMutRef<'a, 'b, 'c, T, BUCKET_SIZE, A, S>, ()> {
        {
            let mut inner = self.inner.borrow_mut();
            inner.require_resident(self.bucket_id)?;
            inner.acquire_ref();
            inner.make_dirty();
        }

        Ok(unsafe { ObjectMutRef::new(&self.inner, self.ptr.as_mut().unwrap()) })
    }
}

impl<'a, 'b, T, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule> Drop
    for Object<'_, '_, T, BUCKET_SIZE, A, S>
{
    fn drop(&mut self) {
        self.inner
            .borrow_mut()
            .drop_and_deallocate(self.bucket_id, self.ptr)
            .unwrap();
    }
}

pub struct ObjectRef<
    'a,
    'b,
    'c,
    T: Sized,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    inner: &'a RefCell<MemoryManagerInner<'b, BUCKET_SIZE, A, S>>,
    data_ref: &'c T,
}

impl<
        'a,
        'b,
        'c,
        T: Sized,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > ObjectRef<'a, 'b, 'c, T, BUCKET_SIZE, A, S>
{
    pub(crate) unsafe fn new(
        inner: &'a RefCell<MemoryManagerInner<'b, BUCKET_SIZE, A, S>>,
        data_ref: &'c T,
    ) -> Self {
        ObjectRef { inner, data_ref }
    }
}

impl<T: Sized, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule> Deref
    for ObjectRef<'_, '_, '_, T, BUCKET_SIZE, A, S>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<T: Sized, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule> Drop
    for ObjectRef<'_, '_, '_, T, BUCKET_SIZE, A, S>
{
    fn drop(&mut self) {
        self.inner.borrow_mut().release_ref();
    }
}

pub struct ObjectMutRef<
    'a,
    'b,
    'c,
    T: Sized,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    inner: &'a RefCell<MemoryManagerInner<'b, BUCKET_SIZE, A, S>>,
    data_ref: &'c mut T,
}

impl<
        'a,
        'b,
        'c,
        T: Sized,
        const BUCKET_SIZE: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > ObjectMutRef<'a, 'b, 'c, T, BUCKET_SIZE, A, S>
{
    pub(crate) unsafe fn new(
        inner: &'a RefCell<MemoryManagerInner<'b, BUCKET_SIZE, A, S>>,
        data_ref: &'c mut T,
    ) -> Self {
        ObjectMutRef { inner, data_ref }
    }
}

impl<T: Sized, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule> Deref
    for ObjectMutRef<'_, '_, '_, T, BUCKET_SIZE, A, S>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data_ref
    }
}

impl<T: Sized, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule> DerefMut
    for ObjectMutRef<'_, '_, '_, T, BUCKET_SIZE, A, S>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_ref
    }
}

impl<T: Sized, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule> Drop
    for ObjectMutRef<'_, '_, '_, T, BUCKET_SIZE, A, S>
{
    fn drop(&mut self) {
        self.inner.borrow_mut().release_ref();
    }
}
