
use std::{alloc::Layout, cell::RefCell, rc::Rc};
use crate::{allocation_options::AllocationOptions, modules::{allocator::AllocatorModule, page_replacement::PageReplacementModule, page_storage::PageStorageModule}, vnv_meta_store::{AllocationIdentifier, VNVMetaStore}, vnv_object::VNVObject};

pub struct VNVHeap<A: AllocatorModule + 'static, R: PageReplacementModule, S: PageStorageModule> {
    inner: Rc<RefCell<VNVHeapInner<A, R, S>>>
}

impl<'a, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule> VNVHeap<A, R, S> {
    pub fn new(page_replacement: R, page_storage: S) -> Self {
        VNVHeap {
            inner: Rc::new(RefCell::new(VNVHeapInner {
                meta_store: VNVMetaStore::new(),
                page_replacement_module: page_replacement,
                page_storage_module: page_storage    
            }))
        }
    }

    pub fn allocate<T: Sized>(&self, initial_value: T) -> VNVObject<T, A, R, S> {
        let mut inner = self.inner.borrow_mut();
        let allocation_options = AllocationOptions::new(Some(initial_value));
        let identifier = unsafe { inner.allocate(allocation_options) };

        VNVObject::new(Rc::clone(&self.inner), identifier)
    }
}

pub(crate) struct VNVHeapInner<A: AllocatorModule + 'static, R: PageReplacementModule, S: PageStorageModule> {
    meta_store: VNVMetaStore<A, S>,
    page_replacement_module: R,
    page_storage_module: S
}

impl<'a, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule> VNVHeapInner<A, R, S> {
    pub(crate) unsafe fn allocate<T>(&mut self, options: AllocationOptions<T>) -> AllocationIdentifier<A>{
        self.meta_store.allocate(options, &mut self.page_storage_module)
    }

    pub(crate) unsafe fn deallocate(&mut self, layout: &Layout, identifier: &AllocationIdentifier<A>) {
        self.meta_store.deallocate(layout, &mut self.page_storage_module, identifier);
    }

    pub(crate) unsafe fn get_mut<T>(&mut self, identifier: &AllocationIdentifier<A>) -> *mut T {
        self.meta_store.get_mut(identifier, &mut self.page_storage_module)
    }

    pub(crate) unsafe fn get_ref<T>(&mut self, identifier: &AllocationIdentifier<A>) -> *const T{
        self.meta_store.get_ref(identifier, &mut self.page_storage_module)
    }

    pub(crate) unsafe fn release_mut<T>(&mut self, identifier: &AllocationIdentifier<A>, data: &mut T) {
        self.meta_store.release_mut(identifier, data);
    }

    pub(crate) unsafe fn release_ref<T>(&mut self, identifier: &AllocationIdentifier<A>, data: &T) {
        self.meta_store.release_ref(identifier, data);
    }
}
