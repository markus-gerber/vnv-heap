use log::trace;
use try_lock::TryLock;

use crate::{
    allocation_identifier::AllocationIdentifier, modules::{
        allocator::AllocatorModule,
        nonresident_allocator::NonResidentAllocatorModule,
        object_management::ObjectManagementModule,
        persistent_storage::{
            persistent_storage_util::write_storage_data, PersistentStorageModule,
            SharedStorageReference,
        },
    }, persist_access_point::PersistAccessPoint, resident_object_manager::{
        resident_list::ResidentList,
        resident_object_backup::{calc_backup_obj_layout_static, calc_backup_obj_user_data_offset},
        resident_object_metadata::ResidentObjectMetadata,
        ResidentObjectManager,
    }, shared_persist_lock::SharedPersistLock, vnv_list::VNVList, vnv_object::VNVObject, VNVArray, VNVConfig
};
use core::{
    cell::RefCell,
    marker::PhantomData,
    mem::{size_of, ManuallyDrop},
    sync::atomic::AtomicBool,
};

static mut PERSIST_ACCESS_POINT: PersistAccessPoint = PersistAccessPoint::empty();

/// For test environment we want to wait until a new heap can be created
#[cfg(test)]
static PERSIST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug)]
pub struct LayoutInfo {
    pub cutoff_size: usize,
    pub resident_object_metadata: usize,
    pub object_dirty_size: usize,
    pub persist_access_point_size: usize
}

/// Persists all existing heaps.
///
/// If this function is called because of a *power failure* and the operating system tries to save the systems state
/// calling this function will persist all data, call the OS and block until it is safe to restart execution
/// (e.g. power is back online and all previous state was restored) and restore the state of this heap.
/// In that case this function will only return if all of these steps were executed.
///
/// **Make sure that no other thread of this program is running except for the one running this function!**
pub unsafe fn vnv_persist_all() {
    PERSIST_ACCESS_POINT.persist_if_not_empty();
}

pub(crate) struct ResidentBufPersistentStorage<A: AllocatorModule, S: PersistentStorageModule> {
    resident_list: ResidentList,
    storage_lock: TryLock<()>,
    heap_lock: TryLock<()>,
    persist_queued: AtomicBool,
    storage: S,
    heap: A,
}

pub const fn calc_resident_buf_cutoff_size<A: AllocatorModule, S: PersistentStorageModule>() -> usize
{
    size_of::<ResidentBufPersistentStorage<A, S>>() + size_of::<usize>()
}

pub(crate) const fn calc_resident_buf_default_dirty_size<
    A: AllocatorModule,
    S: PersistentStorageModule,
>() -> usize {
    size_of::<ResidentBufPersistentStorage<A, S>>() + size_of::<usize>()
}

pub struct VNVHeap<
    'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
    S: PersistentStorageModule + 'static,
> {
    inner: ManuallyDrop<RefCell<VNVHeapInner<'a, A, N, M>>>,

    cutoff_ptr: *mut ResidentBufPersistentStorage<A, S>,

    /// For test environment we want to wait until a new heap can be created
    #[cfg(test)]
    _mutex_guard: std::sync::MutexGuard<'static, ()>,
}

impl<
        'a,
        A: AllocatorModule + 'static,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule + 'static,
    > VNVHeap<'a, A, N, M, S>
{
    pub fn new(
        resident_buffer: &'a mut [u8],
        storage_module: S,
        heap: A,
        mut config: VNVConfig,
        persist_handler: fn(*mut u8, usize) -> (),
    ) -> Result<Self, ()> {
        assert!(
            resident_buffer.len() >= config.max_dirty_bytes,
            "dirty size has to be smaller or equal to the resident buffer"
        );

        // for test environment wait until new heap can be created
        // (until PERSIST_ACCESS_POINT is unset)
        #[cfg(test)]
        let mutex_guard = PERSIST_MUTEX.lock().map_err(|_| {
            panic!("Error while locking PERSIST_MUTEX! This normally happens if one thread panics and still has access to a VNVHeap!");
        })?;

        let cutoff_ptr =
            (&mut resident_buffer[0] as *mut u8) as *mut ResidentBufPersistentStorage<A, S>;
        let (
            cutoff,
            resident_buffer,
            mut storage_reference,
            resident_list,
            heap,
            heap_lock,
            persist_queued,
        ) = Self::prepare_access_point(heap, resident_buffer, storage_module)?;

        log::info!("Prepared access point with cutoff={} bytes", cutoff);

        assert!(
            config.max_dirty_bytes >= calc_resident_buf_default_dirty_size::<A, S>(),
            "dirty size is too small!"
        );
        config.max_dirty_bytes -= calc_resident_buf_default_dirty_size::<A, S>();
        if config.max_dirty_bytes > resident_buffer.len() {
            config.max_dirty_bytes = resident_buffer.len();
        }

        unsafe {
            PERSIST_ACCESS_POINT.set(
                &mut resident_buffer[0] as *mut u8,
                resident_buffer.len(),
                resident_list.get_shared_ref(),
                storage_reference
                    .try_lock_clone()
                    .expect("should not fail: not locked yet"),
                persist_handler,
                heap_lock,
                persist_queued,
                *heap.try_lock().unwrap(),
            )?
        }

        let resident_object_manager = ResidentObjectManager::<A, M>::new(
            resident_buffer,
            config.max_dirty_bytes,
            resident_list,
            heap,
        )?;

        // persist() needs one usize to specify its slice size
        let non_resident_offset = config.max_dirty_bytes + size_of::<usize>();
        let mut non_resident_allocator = N::new();
        non_resident_allocator.init(
            non_resident_offset,
            storage_reference.get_max_size() - non_resident_offset,
            &mut storage_reference,
        )?;

        Ok(VNVHeap {
            inner: ManuallyDrop::new(RefCell::new(VNVHeapInner {
                storage_reference,
                resident_object_manager,
                non_resident_allocator,
                _phantom_data: PhantomData,
            })),
            cutoff_ptr,

            #[cfg(test)]
            _mutex_guard: mutex_guard,
        })
    }

    fn prepare_access_point(
        heap: A,
        resident_buffer: &'a mut [u8],
        storage_module: S,
    ) -> Result<
        (
            usize,
            &mut [u8],
            SharedStorageReference,
            &mut ResidentList,
            SharedPersistLock<*mut A>,
            &TryLock<()>,
            &AtomicBool,
        ),
        (),
    > {
        assert!(
            resident_buffer.len() >= calc_resident_buf_cutoff_size::<A, S>(),
            "resident buffer is too small!"
        );

        let ptr = (resident_buffer as *mut [u8]) as *mut u8;

        let inner = ResidentBufPersistentStorage {
            heap,
            storage_lock: TryLock::new(()),
            heap_lock: TryLock::new(()),
            persist_queued: AtomicBool::new(false),
            resident_list: ResidentList::new(),
            storage: storage_module,
        };

        // write inner
        let inner_ref = unsafe {
            let ptr = ptr as *mut ResidentBufPersistentStorage<A, S>;
            ptr.write(inner);
            ptr.as_mut().unwrap()
        };

        // note: persist() needs one usize to specify its slice size
        // calc_resident_buf_cutoff_size > ResidentBufPersistentStorage

        Ok((
            calc_resident_buf_cutoff_size::<A, S>(),
            &mut resident_buffer[calc_resident_buf_cutoff_size::<A, S>()..],
            SharedStorageReference::new(SharedPersistLock::new(
                &mut inner_ref.storage,
                &inner_ref.persist_queued,
                &inner_ref.storage_lock,
            )),
            &mut inner_ref.resident_list,
            SharedPersistLock::new(
                &mut inner_ref.heap,
                &inner_ref.persist_queued,
                &inner_ref.heap_lock,
            ),
            &inner_ref.heap_lock,
            &inner_ref.persist_queued,
        ))
    }

    pub fn allocate<'b, T: Sized + 'b>(
        &'b self,
        initial_value: T,
    ) -> Result<VNVObject<'b, 'a, T, A, N, M>, ()>
    where
        'a: 'b,
    {
        let mut inner = self.inner.borrow_mut();
        let identifier = unsafe { inner.allocate(initial_value, false)? };

        Ok(VNVObject::new(&self.inner, identifier))
    }

    /// pd = partial dirty
    pub fn allocate_pd_array<'b, T: Sized + Copy + 'b, const SIZE: usize>(
        &'b self,
        _initial_value: [T; SIZE],
    ) -> Result<VNVArray<'b, 'a, T, SIZE, A, N, M>, ()>
    where
        'a: 'b,
    {
        panic!("partial dirtiness currently not 100% supported.");

        /*let mut inner = self.inner.borrow_mut();
        let identifier = unsafe { inner.allocate(initial_value, true)? };

        Ok(VNVArray::new(&self.inner, identifier))*/
    }

    pub fn new_list<'b, T: Sized + Clone>(
        &'b self,
    ) -> VNVList<'b, 'a, T, A, N, M>
    where
        'a: 'b,
    {
        VNVList::new(&self.inner)
    }

    /// Returns the size which the `resident_buffer` has to be, so `usable_resident_buffer_size` bytes can be used effectively
    pub const fn calc_resident_buffer_size(usable_resident_buffer_size: usize) -> usize {
        usable_resident_buffer_size + calc_resident_buf_cutoff_size::<A, S>()
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_inner(&self) -> &RefCell<VNVHeapInner<'a, A, N, M>> {
        &self.inner
    }

    pub const fn get_layout_info() -> LayoutInfo {
        LayoutInfo {
            resident_object_metadata: size_of::<ResidentObjectMetadata>(),
            cutoff_size: calc_resident_buf_cutoff_size::<A, S>(),
            object_dirty_size: ResidentObjectMetadata::fresh_object_dirty_size::<()>(false),
            persist_access_point_size: size_of::<PersistAccessPoint>()
        }
    }

    pub fn count_resident_objects<T: Sized>(&self) -> usize {
        let inner = self.inner.borrow();
        inner.count_resident_objects()
    }

}

impl<
        A: AllocatorModule,
        N: NonResidentAllocatorModule,
        M: ObjectManagementModule,
        S: PersistentStorageModule,
    > Drop for VNVHeap<'_, A, N, M, S>
{
    fn drop(&mut self) {
        unsafe {
            PERSIST_ACCESS_POINT.unset().unwrap();
            ManuallyDrop::drop(&mut self.inner);
            self.cutoff_ptr.drop_in_place();
        }
    }
}

pub(crate) struct VNVHeapInner<
    'a,
    A: AllocatorModule,
    N: NonResidentAllocatorModule,
    M: ObjectManagementModule,
> {
    storage_reference: SharedStorageReference<'a, 'a>,
    resident_object_manager: ResidentObjectManager<'a, 'a, A, M>,
    non_resident_allocator: N,
    _phantom_data: PhantomData<A>,
}

impl<'a, A: AllocatorModule, N: NonResidentAllocatorModule, M: ObjectManagementModule>
    VNVHeapInner<'a, A, N, M>
{
    pub(crate) unsafe fn allocate<T: Sized>(
        &mut self,
        initial_value: T,
        use_partial_dirtiness_tracking: bool,
    ) -> Result<AllocationIdentifier<T>, ()> {
        trace!("Allocate new object with {} bytes", size_of::<T>());

        let backup_obj_layout = calc_backup_obj_layout_static::<T>();

        let metadata_offset = self
            .non_resident_allocator
            .allocate(backup_obj_layout, &mut self.storage_reference)?;

        let initial_value = match self.resident_object_manager.try_to_allocate(
            initial_value,
            metadata_offset,
            use_partial_dirtiness_tracking,
        ) {
            Ok(()) => return Ok(AllocationIdentifier::<T>::from_offset(metadata_offset)),
            Err(val) => {
                // could not put this new object into memory
                // write this object now onto persistent storage instead...
                val
            }
        };

        write_storage_data(
            &mut self.storage_reference,
            metadata_offset + calc_backup_obj_user_data_offset(),
            &initial_value,
        )?;
        Ok(AllocationIdentifier::<T>::from_offset(metadata_offset))
    }

    pub(crate) unsafe fn deallocate<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
    ) -> Result<(), ()> {
        trace!(
            "Deallocate object with {} bytes (offset {})",
            size_of::<T>(),
            identifier.offset
        );

        self.resident_object_manager.drop(
            identifier,
            use_partial_dirtiness_tracking,
            &mut self.storage_reference,
        )?;

        let backup_layout = calc_backup_obj_layout_static::<T>();
        self.non_resident_allocator.deallocate(
            identifier.offset,
            backup_layout,
            &mut self.storage_reference,
        )
    }

    pub(crate) unsafe fn get_mut<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
    ) -> Result<*mut T, ()> {
        self.resident_object_manager.get_mut(
            identifier,
            use_partial_dirtiness_tracking,
            &mut self.storage_reference,
        )
    }

    pub(crate) fn flush_object<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
    ) -> Result<(), ()> {
        self.resident_object_manager.flush_object(
            identifier,
            &mut self.storage_reference,
        )
    }

    pub(crate) unsafe fn get_ref<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
    ) -> Result<*const T, ()> {
        self.resident_object_manager.get_ref(
            identifier,
            use_partial_dirtiness_tracking,
            &mut self.storage_reference,
        )
    }

    pub(crate) unsafe fn get_partial_mut<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
    ) -> Result<(*mut ResidentObjectMetadata, *mut T), ()> {
        self.resident_object_manager
            .get_partial_mut(identifier, &mut self.storage_reference)
    }

    pub(crate) fn partial_mut_make_range_dirty(
        &mut self,
        meta_ptr: &mut ResidentObjectMetadata,
        addr_offset: usize,
        size: usize,
    ) -> Result<(), ()> {
        self.resident_object_manager.partial_mut_make_range_dirty(
            meta_ptr,
            addr_offset,
            size,
            &mut self.storage_reference,
        )
    }

    pub(crate) unsafe fn release_partial_mut<T: Sized>(
        &mut self,
        meta_ptr: *mut ResidentObjectMetadata,
    ) {
        self.resident_object_manager
            .release_partial_mut::<T>(meta_ptr)
    }

    pub(crate) unsafe fn release_mut<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) {
        self.resident_object_manager.release_mut(identifier)
    }

    pub(crate) unsafe fn release_ref<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) {
        self.resident_object_manager.release_ref(identifier)
    }

    pub(crate) fn is_resident<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) -> bool {
        self.resident_object_manager.is_resident(identifier)
    }

    pub(crate) fn is_data_dirty<T: Sized>(&mut self, identifier: &AllocationIdentifier<T>) -> bool {
        self.resident_object_manager.is_data_dirty(identifier)
    }

    pub(crate) fn count_resident_objects(&self) -> usize {
        self.resident_object_manager.count_resident_objects()
    }

    pub(crate) fn unload_object<T: Sized>(
        &mut self,
        identifier: &AllocationIdentifier<T>,
        use_partial_dirtiness_tracking: bool,
    ) -> Result<(), ()> {
        self.resident_object_manager.unload_object(
            identifier,
            &mut self.storage_reference,
            use_partial_dirtiness_tracking,
        )
    }
    
    #[cfg(feature = "benchmarks")]
    #[allow(unused)]
    pub(crate) fn get_remaining_dirty_size(&self) -> usize {
        self.resident_object_manager.get_remaining_dirty_size()
    }

    #[cfg(feature = "benchmarks")]
    #[allow(unused)]
    pub(crate) fn get_storage_module(&mut self) -> &mut SharedStorageReference<'a, 'a> {
        &mut self.storage_reference
    }

    #[cfg(feature = "benchmarks")]
    pub(crate) fn get_resident_object_manager(&self) -> &ResidentObjectManager<'a, 'a, A, M> {
        &self.resident_object_manager
    }

    #[cfg(feature = "benchmarks")]
    #[allow(unused)]
    pub(crate) fn get_non_resident_allocator(&self) -> &N {
        &self.non_resident_allocator
    }

    #[cfg(feature = "benchmarks")]
    #[allow(unused)]
    pub(crate) fn get_modules_mut(
        &mut self,
    ) -> (
        &mut SharedStorageReference<'a, 'a>,
        &mut ResidentObjectManager<'a, 'a, A, M>,
        &mut N,
    ) {
        (
            &mut self.storage_reference,
            &mut self.resident_object_manager,
            &mut self.non_resident_allocator,
        )
    }
}
