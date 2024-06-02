use log::trace;

use crate::{
    allocation_identifier::AllocationIdentifier, allocation_options::AllocationOptions, modules::{
        allocator::AllocatorModule, nonresident_allocator::NonResidentAllocatorModule,
        persistent_storage::{persistent_storage_util::write_storage_data, PersistentStorageModule},
    }, persist_access_point::PersistAccessPoint, resident_object_manager::ResidentObjectManager, vnv_object::VNVObject, VNVConfig
};
use core::{alloc::Layout, cell::RefCell, mem::size_of};
use core::marker::PhantomData;

static PERSIST_ACCESS_POINT: PersistAccessPoint = PersistAccessPoint::empty();

/// Persists all existing heaps.
/// 
/// If this function is called because of a *power failure* and the operating system tries to save the systems state
/// calling this function will persist all data, call the OS and block until it is safe to restart execution
/// (e.g. power is back online and all previous state was restored) and restore the state of this heap.
/// In that case this function will only return if all of these steps were executed.
/// 
/// **Make sure that no other thread is running except for the one running this function!**
pub unsafe fn vnv_persist_all() {
    PERSIST_ACCESS_POINT.persist_if_not_empty();
}

pub struct VNVHeap<'a, A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule> {
    inner: RefCell<VNVHeapInner<'a, A, N, S>>,
}

impl<'a, A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule>
    VNVHeap<'a, A, N, S>
{
    pub fn new(resident_buffer: &'a mut [u8], mut storage_module: S, config: VNVConfig) -> Result<Self, ()> {
        assert!(resident_buffer.len() >= config.max_dirty_bytes, "dirty size has to be smaller or equal to the resident buffer");

        let (resident_object_manager, offset) = ResidentObjectManager::<A>::new(resident_buffer, config.max_dirty_bytes, &mut storage_module)?;
        let mut non_resident_allocator = N::new();
        non_resident_allocator.init(offset, storage_module.get_max_size() - offset, &mut storage_module)?;

        Ok(VNVHeap {
            inner: RefCell::new(VNVHeapInner {
                storage_module,
                resident_object_manager,
                non_resident_allocator,
                _phantom_data: PhantomData
            }),
        })
    }

    pub fn allocate<'b, T: Sized + 'b>(&'b self, initial_value: T) -> Result<VNVObject<'b, 'a, T, A, N, S>, ()> where 'a: 'b {
        let mut inner = self.inner.borrow_mut();
        let allocation_options = AllocationOptions::new(initial_value);
        let identifier = unsafe { inner.allocate(allocation_options)? };

        Ok(VNVObject::new(&self.inner, identifier))
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_inner(&self) -> &RefCell<VNVHeapInner<'a, A, N, S>> {
        &self.inner
    }
}

pub(crate) struct VNVHeapInner<
    'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    S: PersistentStorageModule,
> {
    storage_module: S,
    resident_object_manager: ResidentObjectManager<'a, A>,
    non_resident_allocator: N,
    _phantom_data: PhantomData<A>
}

impl<A: AllocatorModule, N: NonResidentAllocatorModule, S: PersistentStorageModule>
    VNVHeapInner<'_, A, N, S>
{
    pub(crate) unsafe fn allocate<T: Sized>(
        &mut self,
        options: AllocationOptions<T>,
    ) -> Result<AllocationIdentifier<T>, ()> {
        trace!("Allocate new object with {} bytes", size_of::<T>());

        let AllocationOptions { layout, initial_value } = options;
        let offset = self.non_resident_allocator.allocate(layout, &mut self.storage_module)?;

        write_storage_data(&mut self.storage_module, offset, &initial_value)?;

        Ok(AllocationIdentifier::<T>::from_offset(offset))
    }

    pub(crate) unsafe fn deallocate<T: Sized>(
        &mut self,
        layout: Layout,
        identifier: &AllocationIdentifier<T>,
    ) -> Result<(), ()> {
        trace!("Deallocate object with {} bytes (offset {})", size_of::<T>(), identifier.offset);

        self.resident_object_manager.drop(identifier, &mut self.non_resident_allocator, &mut self.storage_module)?;
        self.non_resident_allocator.deallocate(identifier.offset, layout, &mut self.storage_module)
    }

    pub(crate) unsafe fn get_mut<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>
    ) -> Result<*mut T, ()> {
        self.resident_object_manager.get_mut(identifier, &mut self.non_resident_allocator, &mut self.storage_module)
    }

    pub(crate) unsafe fn get_ref<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>
    ) -> Result<*const T, ()> {
        self.resident_object_manager.get_ref(identifier, &mut self.non_resident_allocator, &mut self.storage_module)
    }

    pub(crate) unsafe fn release_mut<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        data: &mut T,
    ) {
        self.resident_object_manager.release_mut(identifier, data)
    }

    pub(crate) unsafe fn release_ref<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        data: &T,
    ) {
        self.resident_object_manager.release_ref(identifier, data)
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_storage_module(&mut self) -> &mut S {
        &mut self.storage_module
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_resident_object_manager(&self) -> &ResidentObjectManager<A> {
        &self.resident_object_manager
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_non_resident_allocator(&self) -> &N {
        &self.non_resident_allocator
    }
}
