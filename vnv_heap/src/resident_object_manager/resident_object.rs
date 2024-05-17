use core::{
    alloc::Layout,
    mem::size_of,
    ptr::{null_mut, slice_from_raw_parts},
};

use memoffset::offset_of;

use crate::util::repr_c_layout;

/// A object that is currently stored in RAM
///
/// **IMPORTANT**: DO NOT REMOVE `#[repr(C)]` OR REORDER THE
/// FIELDS AS THESE ARE CRUCIAL FOR THIS IMPLEMENTATION!
#[repr(C)]
pub(super) struct ResidentObject<T: Sized> {
    pub(super) metadata: ResidentObjectMetadata,
    pub(super) data: T,
}

impl<T: Sized> ResidentObject<T> {

    #[inline]
    pub(super) fn to_data_ptr(ptr: *mut ResidentObject<T>) -> *mut T {
        let offset = offset_of!(ResidentObject<T>, data);
        let t_ptr = unsafe { (ptr as *mut u8).offset(offset as isize) };
        t_ptr as *mut T
    }
}

#[inline]
pub(super) fn calc_resident_obj_layout(data_layout: Layout) -> Layout {
    // get layout of ResidentObject
    repr_c_layout(&[Layout::new::<ResidentObjectMetadata>(), data_layout.clone()]).unwrap()
}

pub(super) struct ResidentObjectMetadata {
    /// Actual metadata
    pub(super) inner: ResidentObjectMetadataInner,

    /// Next item in the resident object list
    pub(super) next_resident_object: *mut ResidentObjectMetadata,

    /// Next item in the next dirty object list
    pub(super) next_dirty_object: *mut ResidentObjectMetadata,
}

pub(super) struct ResidentObjectMetadataInner {
    /// Is this object copy newer than on persistent storage
    pub(super) is_dirty: bool,

    /// Counts the amount of references that are currently held
    /// be the program
    pub(super) ref_cnt: usize,

    pub(super) offset: usize,

    pub(super) layout: Layout,

    /// Used to test that `dynamic_metadata_to_data_range` is correct
    /// 
    /// Use `usize::MAX` to disable. This is used when the state will
    /// be restored after a PFI, because VNVHeap has no idea what type
    /// belongs to which metadata.
    #[cfg(debug_assertions)]
    pub(super) data_offset: usize,
}

impl ResidentObjectMetadataInner {
    pub(super) fn new<T: Sized>(offset: usize) -> Self {
        ResidentObjectMetadataInner {
            is_dirty: false,
            ref_cnt: 0,
            layout: Layout::new::<T>(),
            offset,

            #[cfg(debug_assertions)]
            data_offset: offset_of!(ResidentObject<T>, data),
        }
    }

    #[inline]
    pub(super) unsafe fn to_resident_obj_ptr<T>(&mut self) -> *mut ResidentObject<T> {
        ResidentObjectMetadataInner::ptr_to_resident_obj_ptr(self)
    }

    #[inline]
    pub(super) unsafe fn ptr_to_resident_obj_ptr<T>(ptr: *mut ResidentObjectMetadataInner) -> *mut ResidentObject<T> {
        ResidentObjectMetadata::metadata_to_resident_obj_ptr(ResidentObjectMetadata::ptr_from_meta_inner_mut(ptr))
    }

    /// The same as `ptr_to_resident_obj_ptr` but without type `T`
    #[inline]
    pub(super) unsafe fn ptr_to_resident_obj_ptr_base(ptr: *mut ResidentObjectMetadataInner) -> *mut u8 {
        ResidentObjectMetadata::metadata_to_resident_obj_ptr_base(ResidentObjectMetadata::ptr_from_meta_inner_mut(ptr))
    }

    /// ### Safety
    /// 
    /// This call is only safe to call if this ResidentObjectMetadataInner lives inside a ResidentObjectMetadata and a ResidentObject instance.
    #[inline]
    pub(super) unsafe fn dynamic_metadata_to_data_range(&self) -> &[u8] {
        let meta_ptr = (ResidentObjectMetadata::ptr_from_meta_inner(self) as *const u8)
            .add(size_of::<ResidentObjectMetadata>());

        // align base pointer (add alignment, because T could be aligned)
        let base_ptr = ((meta_ptr as usize) + (self.layout.align() - 1)) & !(self.layout.align() - 1);

        // convert back to pointer
        let base_ptr = base_ptr as *const u8;

        // test if the right offset was applied
        #[cfg(debug_assertions)]
        {
            // check that data offset was not disabled
            if self.data_offset != usize::MAX {
                debug_assert_eq!(
                    (ResidentObjectMetadata::ptr_from_meta_inner(self) as *const u8).add(self.data_offset),
                    base_ptr,
                    "Results in an error if the formula for manually getting the address of the data is wrong"
                );
            }
        }

        slice_from_raw_parts(base_ptr, self.layout.size())
            .as_ref()
            .unwrap()
    }
}

impl ResidentObjectMetadata {
    pub(super) fn new<T: Sized>(offset: usize) -> Self {
        ResidentObjectMetadata {
            next_dirty_object: null_mut(),
            next_resident_object: null_mut(),
            inner: ResidentObjectMetadataInner::new::<T>(offset)
        }
    }

    /// Reinterpret metadata as whole resident object.
    ///
    /// ### Safety
    ///
    /// It is only safe to read/write the returned pointer if I you previously create a `ResidentObject` and
    /// are now calling this function on its `metadata` member.
    ///
    /// Also: You should not have any other mutable references to this `ResidentObject`
    #[inline]
    pub(super) unsafe fn metadata_to_resident_obj_ptr<T>(ptr: *mut ResidentObjectMetadata) -> *mut ResidentObject<T> {
        ptr as *mut ResidentObject<T>
    }
    
    /// The same as `metadata_to_resident_obj_ptr` but without the type `T`
    #[inline]
    pub(super) unsafe fn metadata_to_resident_obj_ptr_base(ptr: *mut ResidentObjectMetadata) -> *mut u8 {
        ptr as *mut u8
    }

    #[inline]
    pub(super) unsafe fn ptr_from_meta_inner_mut(ptr: *mut ResidentObjectMetadataInner) -> *mut ResidentObjectMetadata {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, inner);
        (unsafe { (ptr as *mut u8).sub(OFFSET) }) as *mut ResidentObjectMetadata
    }
    #[inline]
    pub(super) unsafe fn ptr_from_meta_inner(ptr: *const ResidentObjectMetadataInner) -> *const ResidentObjectMetadata {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, inner);
        (unsafe { (ptr as *mut u8).sub(OFFSET) }) as *mut ResidentObjectMetadata
    }

    #[inline]
    pub(super) fn get_next_resident_item(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut *mut ResidentObjectMetadata {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, next_resident_object);
        (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut *mut ResidentObjectMetadata
    }

    #[inline]
    pub(super) fn get_next_dirty_item(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut *mut ResidentObjectMetadata {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, next_dirty_object);
        (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut *mut ResidentObjectMetadata
    }

    #[inline]
    pub(super) fn get_inner(
        ptr: *mut ResidentObjectMetadata,
    ) -> *mut ResidentObjectMetadataInner {
        const OFFSET: usize = offset_of!(ResidentObjectMetadata, inner);
        (unsafe { (ptr as *mut u8).add(OFFSET) }) as *mut ResidentObjectMetadataInner
    }
}

#[cfg(test)]
mod test {
    use core::{alloc::Layout, mem::size_of};

    use crate::resident_object_manager::resident_object::{calc_resident_obj_layout, ResidentObject, ResidentObjectMetadata};

    #[test]
    fn test_calc_resident_obj_layout() {
        test_calc_resident_obj_layout_internal::<usize>();
        test_calc_resident_obj_layout_internal::<u8>();

        struct Test1 {
            _a: usize,
            _b: bool,
            _c: usize,
            _d: bool,
            _e: usize
        }
        test_calc_resident_obj_layout_internal::<Test1>();
        
        #[repr(C)]
        struct Test2 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize
        }
        test_calc_resident_obj_layout_internal::<Test2>();
        
        #[repr(C, align(64))]
        struct Test3 {
            a: usize,
            b: bool,
            c: usize,
            d: bool,
            e: usize
        }
        test_calc_resident_obj_layout_internal::<Test3>();
    }

    fn test_calc_resident_obj_layout_internal<T: Sized>() {
        assert_eq!(Layout::new::<ResidentObject<T>>(), calc_resident_obj_layout(Layout::new::<T>()));
    }

    #[test]
    fn test_dynamic_metadata_to_data_range_1() {
        #[derive(Default)]
        struct TestData1 {
            _a: u64,
            _b: bool,
            _c: u16
        }
        test_dynamic_metadata_to_data_range_internal::<TestData1>();

        #[derive(Default)]
        #[repr(C, align(64))]
        struct TestData2 {
            b: bool,
            c: u16
        }
        test_dynamic_metadata_to_data_range_internal::<TestData2>();

        #[derive(Default)]
        struct TestData3 {
            _b: bool,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData3>();

        #[derive(Default)]
        struct TestData4 {
            _a: u128,
            _b: u128,
            _c: u128,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData4>();

        #[derive(Default)]
        struct TestData5 {
            _a: u128,
            _b: u128,
            _d: bool,
            _c: u128,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData4>();

        #[derive(Default)]
        #[repr(C, align(16))]
        struct TestData6 {
            b: bool,
        }
        test_dynamic_metadata_to_data_range_internal::<TestData6>();

    }

    fn test_dynamic_metadata_to_data_range_internal<T: Default>() {
        let obj = ResidentObject {
            metadata: ResidentObjectMetadata::new::<T>(0),
            data: T::default()
        };
        let original_ptr = (&obj.data) as *const T;

        let data_range = unsafe {
            obj.metadata.inner.dynamic_metadata_to_data_range()
        };

        assert_eq!(original_ptr as *const u8, data_range.as_ptr());
        assert_eq!(size_of::<T>(), data_range.len());
    }
}