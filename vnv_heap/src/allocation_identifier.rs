use core::marker::PhantomData;
use std::usize;

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

    pub(crate) fn new_invalid() -> Self {
        Self {
            _phantom_data: PhantomData,
            offset: usize::MAX
        }
    }

    pub(crate) fn is_invalid(&self) -> bool {
        self.offset == usize::MAX
    }
}

impl<T: Sized> Clone for AllocationIdentifier<T> {
    fn clone(&self) -> Self {
        Self { offset: self.offset.clone(), _phantom_data: self._phantom_data.clone() }
    }
}
