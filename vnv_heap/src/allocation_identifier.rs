use std::marker::PhantomData;



pub(crate) struct AllocationIdentifier<T: Sized> {
    offset: usize,
    _phantom_data: PhantomData<T>
}