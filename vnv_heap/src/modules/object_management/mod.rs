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

pub(crate) struct ResidentItemListArguments<'a, 'b, S: PersistentStorageModule, A: AllocatorModule>
{
    pub(crate) storage: &'a mut S,

    pub(crate) allocator: &'a SharedPersistLock<'b, *mut A>,

    pub(crate) remaining_dirty_size: &'a mut usize,
}

pub struct ResidentIterItem<'a, 'b, 'c, 'd, 'e, 'f, S: PersistentStorageModule, A: AllocatorModule>
{
    phantom_data: PhantomData<&'d ()>,
    delete_handle: DeleteHandle<'e, 'f>,
    arguments: &'c mut ResidentItemListArguments<'a, 'b, S, A>,
}

impl<S: PersistentStorageModule, A: AllocatorModule>
    ResidentIterItem<'_, '_, '_, '_, '_, '_, S, A>
{
    /// Unloads this object and checks if `layout` can be allocated now
    pub fn unload_and_check_for_space(self, layout: &Layout) -> Result<bool, ()> {
        unsafe {
            ResidentObjectMetadata::unload_resident_object_dynamic(
                self.delete_handle,
                self.arguments.storage,
                self.arguments.allocator,
                self.arguments.remaining_dirty_size,
            )
        }?;

        {
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
        }

        Ok(false)
    }

    #[inline]
    pub fn is_user_data_dirty(&mut self) -> bool {
        self.delete_handle
            .get_element()
            .inner
            .status
            .is_data_dirty()
    }
}

pub struct ResidentIter<'a, 'b, 'c, 'd, S: PersistentStorageModule, A: AllocatorModule> {
    arguments: &'c mut ResidentItemListArguments<'a, 'b, S, A>,
    iter: IterMut<'d>,
}

impl<'a, 'b, 'c, S: PersistentStorageModule, A: AllocatorModule>
    ResidentIter<'a, 'b, '_, 'c, S, A>
{
    pub fn next<'d>(&'d mut self) -> Option<ResidentIterItem<'a, 'b, '_, '_, 'c, 'd, S, A>> {
        while let Some(mut item) = self.iter.next() {
            if !item.get_element().inner.status.is_in_use() {
                // object found that is not in use

                unsafe {
                    let ptr: DeleteHandle<'c, 'd> = core::mem::transmute(item);

                    return Some(ResidentIterItem {
                        arguments: self.arguments,
                        delete_handle: ptr,
                        phantom_data: PhantomData,
                    });
                }
            } else {
                // object is in use
                // keep on searching
            }
        }
        // no objects left in that iterator
        return None;
    }
}

pub struct ResidentItemList<'a, 'b, 'c, 'd, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) arguments: &'c mut ResidentItemListArguments<'a, 'b, S, A>,
    pub(crate) resident_list: &'d mut ResidentList,
}

impl<'a, 'b, S: PersistentStorageModule, A: AllocatorModule>
    ResidentItemList<'a, 'b, '_, '_, S, A>
{
    pub fn iter(&mut self) -> ResidentIter<'a, 'b, '_, '_, S, A> {
        ResidentIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
        }
    }
}

pub(crate) struct DirtyItemListArguments<'a, 'b, A: AllocatorModule, S: PersistentStorageModule> {
    pub(crate) storage: &'a mut S,
    pub(crate) remaining_dirty_size: &'a mut usize,
    pub(crate) allocator: &'a SharedPersistLock<'b, *mut A>,
}

pub struct DirtyIterItem<'a, 'b, 'c, 'd, 'e, 'f, A: AllocatorModule, S: PersistentStorageModule> {
    arguments: &'c mut DirtyItemListArguments<'a, 'b, A, S>,
    list: PhantomData<&'d ()>,
    delete_handle: DeleteHandle<'e, 'f>,
}

impl<A: AllocatorModule, S: PersistentStorageModule> DirtyIterItem<'_, '_, '_, '_, '_, '_, A, S> {
    #[inline]
    pub fn is_unused(&mut self) -> bool {
        !self.delete_handle.get_element().inner.status.is_in_use()
    }

    /// Unloads this object and returns the amount of additional dirty bytes that are free now
    #[inline]
    pub fn unload(mut self) -> Result<usize, ()> {
        if !self.is_unused() {
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
        let dirty_size = unsafe {
            self.delete_handle
                .get_element()
                .persist_user_data_dynamic(self.arguments.storage)
        }?;
        *self.arguments.remaining_dirty_size += dirty_size;
        Ok(dirty_size)
    }

    #[inline]
    pub fn is_user_data_dirty(&mut self) -> bool {
        self.delete_handle
            .get_element()
            .inner
            .status
            .is_data_dirty()
    }
}

pub struct DirtyIter<'a, 'b, 'c, 'd, A: AllocatorModule, S: PersistentStorageModule> {
    arguments: &'c mut DirtyItemListArguments<'a, 'b, A, S>,
    iter: IterMut<'d>,
}

impl<'a, 'b, 'c, A: AllocatorModule, S: PersistentStorageModule> DirtyIter<'a, 'b, '_, 'c, A, S> {
    pub fn next<'d>(&'d mut self) -> Option<DirtyIterItem<'a, 'b, '_, '_, 'c, 'd, A, S>> {
        while let Some(mut item) = self.iter.next() {
            let status = &item.get_element().inner.status;
            if !status.is_in_use() || !status.is_mutable_ref_active() {
                // object found that is not in use or only
                // has some immutable references

                unsafe {
                    let ptr: DeleteHandle<'c, 'd> = core::mem::transmute(item);

                    return Some(DirtyIterItem {
                        arguments: self.arguments,
                        delete_handle: ptr,
                        list: PhantomData,
                    });
                }
            } else {
                // object currently has mutable reference
                // keep on searching
            }
        }
        // no objects left in that iterator
        return None;
    }
}

pub struct DirtyItemList<'a, 'b, 'c, 'd, A: AllocatorModule, S: PersistentStorageModule> {
    pub(crate) arguments: &'c mut DirtyItemListArguments<'a, 'b, A, S>,
    pub(crate) resident_list: &'d mut ResidentList,
}

impl<'a, 'b, A: AllocatorModule, S: PersistentStorageModule> DirtyItemList<'a, 'b, '_, '_, A, S> {
    pub fn iter(&mut self) -> DirtyIter<'a, 'b, '_, '_, A, S> {
        DirtyIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
        }
    }
}

pub trait ObjectManagementModule {
    fn new() -> Self;

    fn sync_dirty_data<A: AllocatorModule, S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        dirty_item_list: DirtyItemList<'_, '_, '_, '_, A, S>,
    ) -> Result<(), ()>;

    fn unload_objects<S: PersistentStorageModule, A: AllocatorModule>(
        &mut self,
        layout: &Layout,
        resident_item_list: ResidentItemList<S, A>,
    ) -> Result<(), ()>;
}
