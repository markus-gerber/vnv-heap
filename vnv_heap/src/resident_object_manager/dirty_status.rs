const DATA_DIRTY: u8 = 0;
const GENERAL_METADATA_DIRTY: u8 = 1;

#[derive(Clone, Copy)]
pub(crate) struct DirtyStatus {
    bit_list: u8,
}

macro_rules! generate_functions {
    ($bit: ident, $get_name: ident, $set_name: ident) => {
        #[inline]
        pub(crate) fn $get_name(&self) -> bool {
            self.is_set($bit)
        }

        #[inline]
        pub(crate) fn $set_name(&mut self, is_dirty: bool) {
            self.set($bit, is_dirty);
        }
    };
}

impl DirtyStatus {
    /// The whole metadata is dirty except from the data
    pub(crate) const fn new_metadata_dirty() -> DirtyStatus {
        DirtyStatus {
            bit_list: 1 << GENERAL_METADATA_DIRTY
        }
    }

    #[inline]
    fn is_set(&self, bit: u8) -> bool {
        (self.bit_list & (1 << bit)) != 0
    }

    #[inline]
    fn set(&mut self, bit: u8, state: bool) {
        if state {
            // set
            self.bit_list |= 1 << bit;
        } else {
            // unset
            self.bit_list &= !(1 << bit);
        }
    }

    generate_functions!(DATA_DIRTY, is_data_dirty, set_data_dirty);
    generate_functions!(
        GENERAL_METADATA_DIRTY,
        is_general_metadata_dirty,
        set_general_metadata_dirty
    );
}
