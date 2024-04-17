use crate::{
    allocation_options::AllocationOptions,
    modules::{
        allocator::AllocatorModule, memory_provider::MemoryProviderModule,
        page_replacement::PageReplacementModule, page_storage::PageStorageModule,
    },
    vnv_meta_store::{AllocationIdentifier, VNVMetaStore},
    vnv_object::VNVObject,
    vnv_resident_heap::VNVResidentHeap,
    vnv_resident_heap_manager::VNVResidentHeapManagerConfig,
};
use std::{alloc::Layout, cell::RefCell, rc::Rc};

pub struct VNVHeap<
    A: AllocatorModule + 'static,
    R: PageReplacementModule,
    S: PageStorageModule,
    M: MemoryProviderModule,
> {
    // TODO remove the use of std allocator
    inner: RefCell<VNVHeapInner<A, R, S, M>>,
}

impl<
        'a,
        A: AllocatorModule,
        R: PageReplacementModule,
        S: PageStorageModule,
        M: MemoryProviderModule,
    > VNVHeap<A, R, S, M>
{
    pub fn new(page_replacement: R, page_storage: S, config: VNVResidentHeapManagerConfig) -> Self {
        VNVHeap {
            inner: RefCell::new(VNVHeapInner {
                meta_store: VNVMetaStore::new(page_storage, config),
                page_replacement_module: page_replacement,
            }),
        }
    }

    pub fn allocate<T: Sized>(&self, initial_value: T) -> VNVObject<T, A, R, S, M> {
        let mut inner = self.inner.borrow_mut();
        let allocation_options = AllocationOptions::new(initial_value);
        let identifier = unsafe { inner.allocate(allocation_options) };

        VNVObject::new(&self.inner, identifier)
    }
}

pub(crate) struct VNVHeapInner<
    A: AllocatorModule + 'static,
    R: PageReplacementModule,
    S: PageStorageModule,
    M: MemoryProviderModule,
> {
    meta_store: VNVMetaStore<A, S, M>,
    page_replacement_module: R,
}

impl<
        A: AllocatorModule,
        R: PageReplacementModule,
        S: PageStorageModule,
        M: MemoryProviderModule,
    > VNVHeapInner<A, R, S, M>
{
    pub(crate) unsafe fn allocate<T>(
        &mut self,
        options: AllocationOptions<T>,
    ) -> AllocationIdentifier<T, A> {
        self.meta_store.allocate(options)
    }

    pub(crate) unsafe fn deallocate<T>(
        &mut self,
        layout: &Layout,
        identifier: &AllocationIdentifier<T, A>,
    ) {
        self.meta_store.deallocate(layout, identifier);
    }

    pub(crate) unsafe fn get_mut<T>(&mut self, identifier: &AllocationIdentifier<T, A>) -> *mut T {
        self.meta_store.get_mut(identifier)
    }

    pub(crate) unsafe fn get_ref<T>(
        &mut self,
        identifier: &AllocationIdentifier<T, A>,
    ) -> *const T {
        self.meta_store.get_ref(identifier)
    }

    pub(crate) unsafe fn release_mut<T>(
        &mut self,
        identifier: &AllocationIdentifier<T, A>,
        data: &mut T,
    ) {
        self.meta_store.release_mut(identifier, data);
    }

    pub(crate) unsafe fn release_ref<T>(
        &mut self,
        identifier: &AllocationIdentifier<T, A>,
        data: &T,
    ) {
        self.meta_store.release_ref(identifier, data);
    }
}
/*
#[cfg(test)]
mod test {
    use std::{array, cell::RefCell, fmt::Debug};
    use crate::{modules::{allocator::{buddy::BuddyAllocatorModule, AllocatorModule}, page_replacement::{EmptyPageReplacementModule, PageReplacementModule}, page_storage::{mmap::MMapPageStorageModule, PageStorageModule}}, vnv_resident_heap::VNVResidentHeap, vnv_meta_store::test::allocation_identifier_to_heap, vnv_object::{test::obj_to_allocation_identifier, VNVObject}};
    use super::{VNVHeap, VNVHeapInner};

    impl<A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule> VNVHeapInner<A, R, S> {
        pub(crate) unsafe fn unmap_heap(&mut self, heap: *mut VNVResidentHeap<A>) {
            self.meta_store.unmap_heap(heap, &mut self.page_storage_module);
        }
    }

    /// unmaps the heap in which `obj` is stored
    fn unmap_heap<T, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule>(heap: &mut VNVHeap<A, R, S>, obj: &mut VNVObject<T, A, R, S>) {
        let identifier = obj_to_allocation_identifier(obj);
        let heap_manager = allocation_identifier_to_heap(identifier);
        let inner = &mut (*heap.inner).borrow_mut();
        unsafe { inner.unmap_heap(heap_manager) };
    }

    fn test_eq<T: PartialEq + Debug, A: AllocatorModule, R: PageReplacementModule, S: PageStorageModule>(obj: &mut VNVObject<T, A, R, S>, value: T) {
        let obj_ref = obj.get();
        assert_eq!(*obj_ref, value);
    }

    #[test]
    fn test_primitive() {
        let storage = MMapPageStorageModule::new("save_primitive_test.data").unwrap();

        let heap: VNVHeap<BuddyAllocatorModule<16>, EmptyPageReplacementModule, MMapPageStorageModule> = VNVHeap::new(EmptyPageReplacementModule, storage);
        let mut obj = heap.allocate::<u32>(10);

        test_eq(&mut obj, 10);

        {
            let mut mut_ref = obj.get_mut();
            *mut_ref += *mut_ref;
        }

        test_eq(&mut obj, 20);

        {
            let mut mut_ref = obj.get_mut();
            *mut_ref = 230;
        }

        test_eq(&mut obj, 230);
    }

    #[test]
    fn test_custom_struct() {
        const D_LEN: usize = 10;

        #[derive(Debug, PartialEq)]
        struct TestStruct {
            a: u64,
            b: bool,
            c: *mut u8,
            d: [u8; D_LEN],
            e: RefCell<u16>
        }

        let storage = MMapPageStorageModule::new("save_custom_struct_test.data").unwrap();

        let heap: VNVHeap<BuddyAllocatorModule<16>, EmptyPageReplacementModule, MMapPageStorageModule> = VNVHeap::new(EmptyPageReplacementModule, storage);
        let mut c_origin = 8u8;
        let mut obj = heap.allocate::<TestStruct>(TestStruct {
            a: 0,
            b: false,
            c: &mut c_origin,
            d: [0u8; 10],
            e: RefCell::new(1)
        });

        test_eq(&mut obj, TestStruct {
            a: 0,
            b: false,
            c: &mut c_origin,
            d: [0u8; 10],
            e: RefCell::new(1)
        });

        {
            let mut mut_ref = obj.get_mut();
            mut_ref.a = 10;
            mut_ref.b = true;
            for i in 0..mut_ref.d.len() {
                mut_ref.d[i] = (i * 2) as u8;
            }

            *mut_ref.e.borrow_mut() += 2;
        }

        test_eq(&mut obj, TestStruct {
            a: 10,
            b: true,
            c: &mut c_origin,
            d: array::from_fn(|i| (i * 2) as u8),
            e: RefCell::new(3)
        });

    }

    /// same as `test_custom_struct` but unmaps and remaps the data after each operation
    #[test]
    fn test_custom_struct_unmap() {
        const D_LEN: usize = 10;

        #[derive(Debug, PartialEq)]
        struct TestStruct {
            a: u64,
            b: bool,
            c: *mut u8,
            d: [u8; D_LEN],
            e: RefCell<u16>
        }

        let storage = MMapPageStorageModule::new("save_custom_struct_unmap_test.data").unwrap();

        let mut heap: VNVHeap<BuddyAllocatorModule<16>, EmptyPageReplacementModule, MMapPageStorageModule> = VNVHeap::new(EmptyPageReplacementModule, storage);
        let mut c_origin = 8u8;
        let mut obj = heap.allocate::<TestStruct>(TestStruct {
            a: 0,
            b: false,
            c: &mut c_origin,
            d: [0u8; 10],
            e: RefCell::new(1)
        });

        unmap_heap(&mut heap, &mut obj);

        // automatically remaps data
        test_eq(&mut obj, TestStruct {
            a: 0,
            b: false,
            c: &mut c_origin,
            d: [0u8; 10],
            e: RefCell::new(1)
        });

        unmap_heap(&mut heap, &mut obj);

        {
            // automatically remaps data
            let mut mut_ref = obj.get_mut();
            mut_ref.a = 10;
            mut_ref.b = true;
            for i in 0..mut_ref.d.len() {
                mut_ref.d[i] = (i * 2) as u8;
            }

            *mut_ref.e.borrow_mut() += 2;
        }

        unmap_heap(&mut heap, &mut obj);

        // automatically remaps data
        test_eq(&mut obj, TestStruct {
            a: 10,
            b: true,
            c: &mut c_origin,
            d: array::from_fn(|i| (i * 2) as u8),
            e: RefCell::new(3)
        });
    }

}
*/
