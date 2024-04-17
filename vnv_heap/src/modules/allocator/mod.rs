pub mod buddy;

use core::{alloc::Layout, ptr::NonNull};

pub trait AllocatorModule {
    /// Creates a new allocator module object.
    /// 
    /// **Note**: It first will be initialized before it will be used
    fn new() -> Self;

    /// Initializes the allocator module with a memory area
    /// area = [start, start+size)
    ///
    /// returns the new `max_size_hint` (the upper limit of a size limit, excluding alignment)
    unsafe fn init(&mut self, start: *mut u8, size: usize) -> usize;

    /// Allocates new memory
    /// 
    /// returns a pointer to the memory and the new `max_size_hint` (the upper limit of a size limit, excluding alignment)
    unsafe fn allocate(&mut self, layout: &Layout, max_alloc_size: usize) -> Result<(NonNull<u8>, usize), ()>;

    /// Deallocates memory
    /// 
    /// returns the new `max_size_hint` (the upper limit of a size limit, excluding alignment)
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: &Layout, max_alloc_size: usize) -> usize;

    /// Will be called once the base pointer of this heap has changed.
    /// This can happen once a heap has been unmapped and mapped again.
    /// 
    /// **Important**: It is the responsibility of the heap to update all of its pointers or else the whole NVNHeap will break
    unsafe fn on_ptr_change(&mut self, old_base_ptr: *mut u8, new_base_ptr: *mut u8);

    /// Calculates the minimum size requirements that are necessary to allocate `layout`
    /// for a fresh heap. (Asks how big a new heap should be to safely allocate `layout`)
    /// 
    /// **Note**: This calculation should just exclude the size to store the struct of this AllocatorModule.
    fn calc_min_size_for_layout(layout: &Layout) -> usize;
}