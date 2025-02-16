// this file contains the model for the baseline benchmark

use super::{object::Object, AllocatorModule, PersistentStorageModule};
use core::{
    alloc::Layout,
    cell::RefCell,
    mem::size_of,
    ptr::{null_mut, NonNull},
};
use std::cell::RefMut;

pub(crate) struct MemoryManager<
    'a,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    inner: RefCell<MemoryManagerInner<'a, BUCKET_SIZE, A, S>>,
}

impl<'a, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule>
    MemoryManager<'a, BUCKET_SIZE, A, S>
{
    pub(crate) fn new<F: Fn() -> A>(
        buffer: &'a mut [u8; BUCKET_SIZE],
        storage: &'a mut S,
        bucket_count: usize,
        gen_alloc: F,
    ) -> Self {
        Self {
            inner: RefCell::new(MemoryManagerInner::new(
                buffer,
                storage,
                bucket_count,
                gen_alloc,
            )),
        }
    }

    pub(crate) fn allocate<'b, T>(
        &'b self,
        bucket_id: usize,
        data: T,
    ) -> Result<Object<'b, 'a, T, BUCKET_SIZE, A, S>, ()>
    where
        'a: 'b,
    {
        let ptr = self.inner.borrow_mut().allocate(bucket_id, data)?;

        Ok(Object::new(bucket_id, ptr, &self.inner))
    }

    pub(crate) fn bucket_count(&self) -> usize {
        self.inner.borrow().bucket_count
    }

    pub(crate) fn get_inner(&self) -> RefMut<'_, MemoryManagerInner<'a, BUCKET_SIZE, A, S>> {
        self.inner.borrow_mut()
    }
}

pub(crate) struct MemoryManagerInner<
    'a,
    const BUCKET_SIZE: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    buffer: &'a mut [u8; BUCKET_SIZE],
    storage: &'a mut S,

    allocator: *mut A,
    dirty: bool,
    current_bucket: usize,
    bucket_count: usize,
    open_references: usize,
}

impl<'a, const BUCKET_SIZE: usize, A: AllocatorModule, S: PersistentStorageModule>
    MemoryManagerInner<'a, BUCKET_SIZE, A, S>
{
    pub(crate) fn new<F: Fn() -> A>(
        buffer: &'a mut [u8; BUCKET_SIZE],
        storage: &'a mut S,
        bucket_count: usize,
        gen_alloc: F,
    ) -> Self {
        assert!(BUCKET_SIZE > size_of::<A>());
        assert!(BUCKET_SIZE * bucket_count <= storage.get_max_size());

        let mut instance = Self {
            buffer,
            storage,
            bucket_count,
            current_bucket: 0,
            open_references: 0,
            dirty: false,
            allocator: null_mut(),
        };

        for i in 0..bucket_count {
            instance.require_resident(i).unwrap();
            instance.init_heap::<F>(&gen_alloc);
        }

        instance.require_resident(0).unwrap();
        instance.allocator = (instance.buffer as *mut [u8; BUCKET_SIZE]) as *mut A;

        instance
    }

    pub(crate) fn make_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn curr_resident_bucket(&self) -> usize {
        self.current_bucket
    }

    pub(crate) fn allocator(&mut self) -> &mut A {
        unsafe { self.allocator.as_mut().unwrap() }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn init_heap<F: Fn() -> A>(&mut self, gen_alloc: &F) {
        let base_ptr = (self.buffer as *mut [u8; BUCKET_SIZE]) as *mut u8;
        let ptr = base_ptr as *mut A;
        let alloc = gen_alloc();
        unsafe { ptr.write(alloc) };

        let alloc = unsafe { ptr.as_mut().unwrap() };

        let user_data_start = base_ptr.wrapping_add(size_of::<A>());
        unsafe { alloc.init(user_data_start, BUCKET_SIZE - size_of::<A>()) };

        self.dirty = true;
    }

    pub(crate) fn require_resident(&mut self, bucket_id: usize) -> Result<(), ()> {
        if self.current_bucket == bucket_id {
            return Ok(());
        }

        if self.open_references > 0 {
            return Err(());
        }

        if self.dirty {
            self.sync()?;
        }

        let offset = get_storage_offset(bucket_id, BUCKET_SIZE);

        self.storage.read(offset, self.buffer)?;
        self.current_bucket = bucket_id;

        Ok(())
    }

    pub(crate) fn allocate<T>(&mut self, bucket_id: usize, data: T) -> Result<*mut T, ()> {
        if bucket_id >= self.bucket_count {
            return Err(());
        }

        self.require_resident(bucket_id)?;

        let allocator = unsafe { self.allocator.as_mut().unwrap() };
        let ptr = unsafe { allocator.allocate(Layout::new::<T>())? };
        let ptr = (ptr.as_ptr()) as *mut T;

        unsafe { ptr.write(data) };

        self.dirty = true;
        Ok(ptr)
    }

    pub(crate) fn drop_and_deallocate<T>(&mut self, bucket_id: usize, data: *mut T) -> Result<(), ()> {
        if bucket_id >= self.bucket_count {
            return Err(());
        }

        self.require_resident(bucket_id)?;

        unsafe { data.drop_in_place() };

        let allocator = unsafe { self.allocator.as_mut().unwrap() };
        unsafe { allocator.deallocate(NonNull::new(data as *mut u8).unwrap(), Layout::new::<T>()) };

        self.dirty = true;
        Ok(())
    }

    pub(crate) fn acquire_ref(&mut self) {
        self.open_references += 1;
    }

    pub(crate) fn release_ref(&mut self) {
        self.open_references -= 1;
    }

    pub(crate) fn sync(&mut self) -> Result<(), ()> {
        let offset = get_storage_offset(self.current_bucket, BUCKET_SIZE);
        self.storage.write(offset, self.buffer)?;

        self.dirty = false;

        Ok(())
    }
}

fn get_storage_offset(bucket_id: usize, bucket_size: usize) -> usize {
    bucket_id * bucket_size
}
