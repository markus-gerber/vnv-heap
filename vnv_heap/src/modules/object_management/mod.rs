use super::{allocator::AllocatorModule, persistent_storage::PersistentStorageModule};
use crate::{
    resident_object_manager::{
        resident_list::{DeleteHandle, IterMut, ResidentList},
        resident_object_metadata::ResidentObjectMetadata,
    },
    shared_persist_lock::SharedPersistLock,
};
use core::{alloc::Layout, marker::PhantomData};

mod default;
pub use default::*;


pub trait ObjectManagementModule {
    fn new() -> Self;

    fn sync_dirty_data<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        dirty_item_list: ObjectManagementList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()>;

    fn unload_objects<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        layout: &Layout,
        resident_item_list: ObjectManagementList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()>;

    fn access_object(&mut self, _metadata: ObjectStatusWrapper) {}

    fn modify_object(&mut self, _metadata: ObjectStatusWrapper) {}
}


pub struct ObjectStatusWrapper<'a> {
    pub(crate) metadata: &'a mut ResidentObjectMetadata,
}

impl ObjectStatusWrapper<'_> {

    #[inline]
    pub fn is_data_dirty(&self) -> bool {
        self.metadata.inner.status.is_data_dirty()
    }

    #[inline]
    pub fn is_in_use(&self) -> bool {
        self.metadata.inner.status.is_in_use()
    }

    #[inline]
    pub fn is_mutable_ref_active(&self) -> bool {
        self.metadata.inner.status.is_mutable_ref_active()
    }
}


pub(crate) struct ObjectManagementListArguments<'a, 'b, A: AllocatorModule, S: PersistentStorageModule> {
    pub(crate) storage: &'a mut S,
    pub(crate) remaining_dirty_size: &'a mut usize,
    pub(crate) allocator: &'a SharedPersistLock<'b, *mut A>,
}

pub struct ObjectManagementIterItem<'a, 'b, 'c, 'd, 'e, 'f, A: AllocatorModule, S: PersistentStorageModule> {
    arguments: &'c mut ObjectManagementListArguments<'a, 'b, A, S>,
    list: PhantomData<&'d ()>,
    delete_handle: DeleteHandle<'e, 'f>,
}

impl<A: AllocatorModule, S: PersistentStorageModule> ObjectManagementIterItem<'_, '_, '_, '_, '_, '_, A, S> {
    #[inline]
    pub fn get_metadata<'a>(&'a mut self) -> ObjectStatusWrapper<'a> {
        ObjectStatusWrapper { metadata: self.delete_handle.get_element() }
    }

    #[inline]
    pub fn get_ptr(&mut self) -> *const u8 {
        (self.delete_handle.get_element() as *mut _) as *const u8
    }

    /// Unloads this object and checks if `layout` can be allocated now
    #[inline]
    pub fn unload_and_check_for_space(mut self, layout: &Layout) -> Result<bool, ()> {
        if self.get_metadata().is_in_use() {
            return Err(());
        }
        unsafe {
            ResidentObjectMetadata::unload_resident_object_dynamic(
                self.delete_handle,
                self.arguments.storage,
                self.arguments.allocator,
                self.arguments.remaining_dirty_size,
            )
        }?;

        // unwrap is okay here because there are no other threads concurrently accessing it
        // except from vnv_persist_all, but as it is guaranteed that no other threads run
        // during its execution, it is fine
        let guard = self.arguments.allocator.try_lock().unwrap();

        // TODO optimize this by changing deallocate interface
        unsafe {
            if let Ok(ptr) = guard.as_mut().unwrap().allocate(layout.clone()) {
                guard.as_mut().unwrap().deallocate(ptr, layout.clone());
                return Ok(true);
            }
        }

        drop(guard);

        Ok(false)

    }

    /// Unloads this object and returns the amount of additional dirty bytes that are free now
    pub fn unload(mut self) -> Result<usize, ()> {
        if self.get_metadata().is_in_use() {
            return Err(());
        }
        let prev = *self.arguments.remaining_dirty_size;
        unsafe {
            ResidentObjectMetadata::unload_resident_object_dynamic(
                self.delete_handle,
                self.arguments.storage,
                self.arguments.allocator,
                self.arguments.remaining_dirty_size,
            )
        }?;

        Ok(*self.arguments.remaining_dirty_size - prev)
    }

    #[inline]
    pub fn sync_user_data(&mut self) -> Result<usize, ()> {
        if self.get_metadata().is_data_dirty() && self.get_metadata().is_in_use() {
            return Err(());
        }

        let dirty_size = unsafe {
            self.delete_handle
                .get_element()
                .persist_user_data_dynamic(self.arguments.storage)
        }?;
        *self.arguments.remaining_dirty_size += dirty_size;
        Ok(dirty_size)
    }

}

pub struct ObjectManagementIter<'a, 'b, 'c, 'd, A: AllocatorModule, S: PersistentStorageModule> {
    arguments: &'c mut ObjectManagementListArguments<'a, 'b, A, S>,
    iter: IterMut<'d>,
}

impl<'a, 'b, 'c, A: AllocatorModule, S: PersistentStorageModule> ObjectManagementIter<'a, 'b, '_, 'c, A, S> {
    pub fn next<'d>(&'d mut self) -> Option<ObjectManagementIterItem<'a, 'b, '_, '_, 'c, 'd, A, S>> {
        while let Some(mut item) = self.iter.next() {
            unsafe {
                let ptr: DeleteHandle<'c, 'd> = core::mem::transmute(item);

                return Some(ObjectManagementIterItem {
                    arguments: self.arguments,
                    delete_handle: ptr,
                    list: PhantomData,
                });
            }
        }
        // no objects left in that iterator
        return None;
    }
}

pub struct ObjectManagementList<'a, 'b, 'c, 'd, A: AllocatorModule, S: PersistentStorageModule> {
    pub(crate) arguments: &'c mut ObjectManagementListArguments<'a, 'b, A, S>,
    pub(crate) resident_list: &'d mut ResidentList,
}

impl<'a, 'b, A: AllocatorModule, S: PersistentStorageModule> ObjectManagementList<'a, 'b, '_, '_, A, S> {
    pub fn iter(&mut self) -> ObjectManagementIter<'a, 'b, '_, '_, A, S> {
        ObjectManagementIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
        }
    }
}
