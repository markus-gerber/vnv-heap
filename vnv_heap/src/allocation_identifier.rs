use core::marker::PhantomData;

pub(crate) struct AllocationIdentifier<T: Sized> {
    pub(crate) offset: usize,
    _phantom_data: PhantomData<T>
}

impl<T: Sized> AllocationIdentifier<T> {
    pub(crate) fn from_offset(offset: usize) -> Self {
        Self {
            offset,
            _phantom_data: PhantomData
        }
    }
}