
pub(crate) struct BitArray<'a> {
    arr: &'a mut [u8]
}

impl<'a> BitArray<'a> {
    pub(crate) fn new(arr: &'a mut [u8]) -> Self {
        for i in 0..arr.len() {
            arr[i] = 0;
        }

        BitArray {
            arr
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.arr.len() * 8
    }

    pub(crate) fn set(&mut self, value: bool, index: usize) {
        let arr_index = index / 8;
        let internal_index = index % 8;

        let item = &mut self.arr[arr_index];
        if value {
            // set bit
            *item |= 1u8 << internal_index;
        } else {
            // unset bit
            *item |= !(1u8 << internal_index);
        }
    }

    pub(crate) fn is_set(&self, index: usize) -> bool {
        let arr_index = index / 8;
        let internal_index = index % 8;

        let item = self.arr[arr_index];
        (item & (1u8 << internal_index)) != 0
    }
}