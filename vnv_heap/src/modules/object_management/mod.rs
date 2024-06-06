use super::{
    allocator::{AllocatorModule, LinkedListAllocatorModule},
    persistent_storage::{FilePersistentStorageModule, PersistentStorageModule},
};
use crate::resident_object_manager::{
    resident_list::{DeleteHandle, IterMut, ResidentList},
    resident_object::ResidentObjectMetadata,
    MetadataBackupList,
};
use core::alloc::Layout;
use std::{marker::PhantomData, mem::transmute, ptr::null_mut};

mod default;
pub use default::*;

pub(crate) struct ResidentItemListArguments<'a, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) storage: &'a mut S,

    pub(crate) allocator: &'a mut A,

    pub(crate) resident_object_count: &'a mut usize,

    pub(crate) remaining_dirty_size: &'a mut usize,
}

pub struct ResidentIterItem<'a, 'b, 'c, 'd, 'e, S: PersistentStorageModule, A: AllocatorModule> {
    phantom_data: PhantomData<&'c ()>,
    delete_handle: DeleteHandle<'d, 'e>,
    arguments: &'b mut ResidentItemListArguments<'a, S, A>,
}

impl<S: PersistentStorageModule, A: AllocatorModule> ResidentIterItem<'_, '_, '_, '_, '_, S, A> {
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

        *self.arguments.resident_object_count -= 1;

        // TODO optimize this by changing deallocate interface
        unsafe {
            if let Ok(ptr) = self.arguments.allocator.allocate(layout.clone()) {
                self.arguments.allocator.deallocate(ptr, layout.clone());
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub struct ResidentIter<'a, 'b, 'c, S: PersistentStorageModule, A: AllocatorModule> {
    arguments: &'b mut ResidentItemListArguments<'a, S, A>,
    iter: IterMut<'c>,
}

impl<'a, 'c, S: PersistentStorageModule, A: AllocatorModule> ResidentIter<'a, '_, 'c, S, A> {
    pub fn next<'d>(&'d mut self) -> Option<ResidentIterItem<'a, '_, '_, 'c, 'd, S, A>> {
        while let Some(mut item) = self.iter.next() {
            if item.get_element().inner.ref_cnt == 0 {
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

pub struct ResidentItemList<'a, 'b, 'c, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) arguments: &'b mut ResidentItemListArguments<'a, S, A>,
    pub(crate) resident_list: &'c mut ResidentList,
}

impl<'a, S: PersistentStorageModule, A: AllocatorModule> ResidentItemList<'a, '_, '_, S, A> {
    pub fn iter(&mut self) -> ResidentIter<'a, '_, '_, S, A> {
        ResidentIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
        }
    }
}

/*
pub(crate) struct ResidentItemListArguments<'a, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) storage: &'a mut S,

    pub(crate) allocator: &'a mut A,

    pub(crate) resident_object_count: &'a mut usize,

    pub(crate) remaining_dirty_size: &'a mut usize,
}

pub struct ResidentIterItem<'a, 'b, 'c: 'a, 'd, S: PersistentStorageModule, A: AllocatorModule> {
    delete_handle: DeleteHandle<'c, 'd>,
    arguments: &'a mut ResidentItemListArguments<'b, S, A>
}

impl<S: PersistentStorageModule, A: AllocatorModule> ResidentIterItem<'_, '_, '_, '_, S, A> {
    /// Unloads this object and checks if `layout` can be allocated now
    pub fn unload_and_check_for_space(self, layout: &Layout) -> Result<bool, ()> {
        unsafe { ResidentObjectMetadata::unload_resident_object_dynamic(self.delete_handle, self.arguments.storage, self.arguments.allocator, self.arguments.remaining_dirty_size) }?;

        *self.arguments.resident_object_count -= 1;

        // TODO optimize this by changing deallocate interface
        unsafe {
            if let Ok(ptr) = self.arguments.allocator.allocate(layout.clone()) {
                self.arguments.allocator.deallocate(ptr, layout.clone());
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub struct ResidentIter<'a, 'b, 'c: 'a, S: PersistentStorageModule, A: AllocatorModule> {
    arguments: &'a mut ResidentItemListArguments<'b, S, A>,
    iter: IterMut<'c>,
    phantom: PhantomData<&'c ()>
}

impl<'a, 'b, 'c: 'a, S: PersistentStorageModule, A: AllocatorModule> ResidentIter<'a, 'b, 'c, S, A> {
    pub fn next(&'a mut self) -> Option<ResidentIterItem<'a, 'b, 'c, 'c, S, A>> {
        while let Some(mut item) = self.iter.next() {
            if item.get_element().inner.ref_cnt == 0 {
                // object found that is not in use
                return Some(ResidentIterItem {
                    arguments: self.arguments,
                    delete_handle: item
                });
            } else {
                // object is in use
                // keep on searching
            }
        }
        // no objects left in that iterator
        return None;
    }
}

pub struct ResidentItemList<'a, 'b, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) arguments: &'a mut ResidentItemListArguments<'b, S, A>,
    pub(crate) resident_list: &'a mut ResidentList,
}

impl<'a, 'b, S: PersistentStorageModule, A: AllocatorModule> ResidentItemList<'a, 'b, S, A> {
    pub fn iter<'c: 'a>(&'c mut self) -> ResidentIter<'a, 'b, 'c, S, A> {
        ResidentIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
            phantom: PhantomData
        }
    }
}
*/
/*
pub(crate) struct ResidentItemListArguments<'a, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) storage: &'a mut S,

    pub(crate) allocator: &'a mut A,

    pub(crate) resident_object_count: &'a mut usize,

    pub(crate) remaining_dirty_size: &'a mut usize,
}

pub struct ResidentIterItem<'a, 'b: 'c, 'c: 'd, 'd, S: PersistentStorageModule, A: AllocatorModule> {
    phantom_data: PhantomData<&'b ()>,
    delete_handle: DeleteHandle<'c, 'd>,
    arguments: &'a mut ResidentItemListArguments<'a, S, A>
}

impl<S: PersistentStorageModule, A: AllocatorModule> ResidentIterItem<'_, '_, '_, '_, S, A> {
    /// Unloads this object and checks if `layout` can be allocated now
    pub fn unload_and_check_for_space(self, layout: &Layout) -> Result<bool, ()> {
        unsafe { ResidentObjectMetadata::unload_resident_object_dynamic(self.delete_handle, self.arguments.storage, self.arguments.allocator, self.arguments.remaining_dirty_size) }?;

        *self.arguments.resident_object_count -= 1;

        // TODO optimize this by changing deallocate interface
        unsafe {
            if let Ok(ptr) = self.arguments.allocator.allocate(layout.clone()) {
                self.arguments.allocator.deallocate(ptr, layout.clone());
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub struct ResidentIter<'a, 'b: 'c, 'c, S: PersistentStorageModule, A: AllocatorModule> {
    arguments: &'a mut ResidentItemListArguments<'a, S, A>,
    iter: IterMut<'c>,
    phantom_data: PhantomData<&'b ()>
}

impl<'a, 'b: 'c, 'c, S: PersistentStorageModule, A: AllocatorModule> ResidentIter<'a, 'b, 'c, S, A> {
    pub fn next<'d: 'a + 'b + 'c>(&'d mut self) -> Option<ResidentIterItem<'a, 'b, 'c, 'd, S, A>> {
        // https://github.com/rust-lang/rust/issues/92985
        use ::polonius_the_crab::prelude::*;
/*
        let iter = &mut self.iter;
        let mut args = &mut self.arguments;

        polonius_loop!(|iter| -> Option<ResidentIterItem<'a, 'b, 'c, S, A>> {
            let item = iter.next();
            if let Some(mut item) = item {
                if item.get_element().inner.ref_cnt == 0 {
                    // object found that is not in use
                    polonius_return!(Some(ResidentIterItem {
                        arguments: args,
                        delete_handle: item
                    }))
                } else {
                    // object is in use
                    // keep on searching
                    polonius_continue!()
                }
            } else {
                // no objects left in that iterator
                polonius_return!(None)
            }
        })
*/
/*
        let mut iter = &mut self.iter;
        let item: Option<DeleteHandle<'_, 'b>> = polonius_loop!(|iter| -> Option<ResidentIterItem<'_, 'polonius, 'polonius, S, A>> {
            let mut ret: Option<DeleteHandle<'_, '_>> = iter.next();
            if let Some(it) = ret.as_mut() {
                if it.get_element().inner.ref_cnt != 0 {
                    polonius_continue!();
                }
            }
            polonius_break!(ret);
        });

        if let Some(item) = item {
            Some(ResidentIterItem {
                arguments: self.arguments,
                delete_handle: item
            })
        } else {
            None
        }
        */
        if let Some(item) = self.iter.next() {
            Some(ResidentIterItem {
                arguments: self.arguments,
                delete_handle: item,
                phantom_data: PhantomData
            })
        } else {
            None
        }

/*
        loop {
            let item: Option<DeleteHandle> = self.iter.next();

            if let Some(mut item) = item {

                if item.get_element().inner.ref_cnt == 0 {
                    // object found that is not in use

                    unsafe {
                        // should be safe as this is similar as in
                        // https://github.com/rust-lang/rust/issues/92985#issuecomment-1014833520

                        return Some(ResidentIterItem {
                            arguments: self.arguments,
                            delete_handle: item
                        });
                    }
                } else {
                    // object is in use
                    // keep on searching
                }
            } else {
                // no objects left in that iterator
                return None;
            }
        }*/
        /*
        while let Some(mut item) = self.iter.next::<'c>() {
            if item.get_element().inner.ref_cnt == 0 {
                // object found that is not in use

                unsafe {
                    let ptr: DeleteHandle<'b, 'c> = core::mem::transmute(item);

                    return Some(ResidentIterItem {
                        arguments: self.arguments,
                        delete_handle: ptr
                    });
                }

            } else {
                // object is in use
                // keep on searching
            }
        }
        // no objects left in that iterator
        return None;
        */
    }
}

pub struct ResidentItemList<'a, 'b, S: PersistentStorageModule, A: AllocatorModule> {
    pub(crate) arguments: &'a mut ResidentItemListArguments<'a, S, A>,
    pub(crate) resident_list: &'b mut ResidentList,
}

impl<'a, 'b, S: PersistentStorageModule, A: AllocatorModule> ResidentItemList<'a, 'b, S, A> {
    pub fn iter<'c: 'a + 'b>(&'c mut self) -> ResidentIter<'a, 'b, 'c, S, A> where 'b: 'c {
        ResidentIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
            phantom_data: PhantomData
        }
    }
}
*/
pub(crate) struct DirtyItemListArguments<'a, S: PersistentStorageModule> {
    pub(crate) storage: &'a mut S,
    pub(crate) resident_object_meta_backup: &'a mut MetadataBackupList,
    pub(crate) remaining_dirty_size: &'a mut usize,
}

pub struct DirtyIterItem<'a, 'b, 'c, 'd, 'e, S: PersistentStorageModule> {
    arguments: &'b mut DirtyItemListArguments<'a, S>,
    list: PhantomData<&'c ()>,
    delete_handle: DeleteHandle<'d, 'e>,
}

impl<S: PersistentStorageModule> DirtyIterItem<'_, '_, '_, '_, '_, S> {
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
    pub fn sync_metadata(&mut self) -> Result<usize, ()> {
        let dirty_size = self.delete_handle.get_element().persist_metadata(
            self.arguments.storage,
            self.arguments.resident_object_meta_backup,
        )?;
        *self.arguments.remaining_dirty_size += dirty_size;
        Ok(dirty_size)
    }

    #[inline]
    pub fn is_user_data_dirty(&mut self) -> bool {
        self.delete_handle
            .get_element()
            .inner
            .dirty_status
            .is_data_dirty()
    }

    #[inline]
    pub fn is_metadata_dirty(&mut self) -> bool {
        self.delete_handle
            .get_element()
            .inner
            .dirty_status
            .is_general_metadata_dirty()
    }
}

pub struct DirtyIter<'a, 'b, 'c, S: PersistentStorageModule> {
    arguments: &'b mut DirtyItemListArguments<'a, S>,
    iter: IterMut<'c>,
}

impl<'a, 'c, S: PersistentStorageModule> DirtyIter<'a, '_, 'c, S> {
    pub fn next<'d>(&'d mut self) -> Option<DirtyIterItem<'a, '_, '_, 'c, 'd, S>> {
        while let Some(mut item) = self.iter.next() {
            if item.get_element().inner.ref_cnt != usize::MAX {
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

        /*
        // https://github.com/rust-lang/rust/issues/92985
        use ::polonius_the_crab::prelude::*;
        if let Some(item) = self.iter.next() {
            Some(DirtyIterItem {
                arguments: self.arguments,
                delete_handle: item,
                list: PhantomData
            })
        } else {
            None
        }*/
        /*
                let mut iter = &mut self.iter;
                let item: Option<DeleteHandle<'b, 'c>> = polonius_loop!(|iter| -> Option<DirtyIterItem<'_, '_, 'polonius, S>> {
                    let mut ret: Option<DeleteHandle<'_, '_>> = iter.next();
                    if let Some(it) = ret.as_mut() {
                        if it.get_element().inner.ref_cnt == usize::MAX {
                            polonius_continue!();
                        }
                    }
                    polonius_break!(ret);
                });

                if let Some(item) = item {
                    Some(DirtyIterItem {
                        arguments: self.arguments,
                        delete_handle: item
                    })
                } else {
                    None
                }
        */
        /*
        loop {
            if let Some(mut item) = self.iter.next() {
                if item.get_element().inner.ref_cnt != usize::MAX {
                    // object found that is not in use or only
                    // has some immutable references
                    return Some(DirtyIterItem {
                        arguments: self.arguments,
                        delete_handle: item
                    });
                } else {
                    // object currently has mutable reference
                    // keep on searching
                }
            } else {
                // no objects left in that iterator
                return None;
            }
        }
        */
    }
}

pub struct DirtyItemList<'a, 'b, 'c, S: PersistentStorageModule> {
    pub(crate) arguments: &'b mut DirtyItemListArguments<'a, S>,
    pub(crate) resident_list: &'c mut ResidentList,
}

impl<'a, S: PersistentStorageModule> DirtyItemList<'a, '_, '_, S> {
    pub fn iter(&mut self) -> DirtyIter<'a, '_, '_, S> {
        DirtyIter {
            arguments: self.arguments,
            iter: self.resident_list.iter_mut(),
        }
    }
}

pub trait ObjectManagementModule {
    fn new() -> Self;

    fn sync_dirty_data<S: PersistentStorageModule>(
        &mut self,
        required_bytes: usize,
        dirty_item_list: DirtyItemList<'_, '_, '_, S>,
    ) -> Result<(), ()>;

    fn unload_objects<S: PersistentStorageModule, A: AllocatorModule>(
        &mut self,
        layout: &Layout,
        resident_item_list: ResidentItemList<S, A>,
    ) -> Result<(), ()>;
}
