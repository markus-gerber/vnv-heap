use core::alloc::Layout;

pub(crate) struct AllocationOptions<T> {
    pub(crate) layout: Layout,
    pub(crate) initial_value: T
}

impl<T> AllocationOptions<T> {
    pub(crate) fn new(initial_value: T) -> Self {
        AllocationOptions {
            layout: Layout::new::<T>(),
            initial_value
        }
    }
}