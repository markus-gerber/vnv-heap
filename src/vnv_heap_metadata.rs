use std::alloc::Layout;

use crate::{modules::allocator::AllocatorModule, vnv_resident_heap::VNVResidentHeap};

/// Metadata for a sub heap of VNVHeap
pub(crate) struct VNVHeapMetadata<A: AllocatorModule + 'static> {
    /// A size hint that hints
    /// the maximum size that still can be allocated.
    ///
    /// Because alignment is not considered, allocation can fail even if `max_size_hint >= size_to_alloc`.
    /// But if `max_size_hint < size_to_alloc` you can be sure that there is no space left.
    /// So this means `max_size_hint` is just an **upper limit** for allocation.
    pub(crate) max_size_hint: usize,

    /// offset of this data in the heap storage
    pub(crate) offset: u64,

    /// size of the total heap, including the metadata
    pub(crate) size: usize,

    /// Pointer to a resident heap.
    ///
    /// Will be `null` if this heap is not resident.
    pub(crate) resident_ptr: *mut VNVResidentHeap<A>,
}

impl<A: AllocatorModule> VNVHeapMetadata<A> {
    /// Checks if this heap has any space left to allocate `layout`
    ///
    /// **Note**: Currently alignment is not considered, so in some cases `has_space_left` could return `true`
    // even though `allocate` will fail
    pub(crate) fn has_space_left(&self, layout: &Layout) -> bool {
        self.max_size_hint >= layout.size()
    }
}
