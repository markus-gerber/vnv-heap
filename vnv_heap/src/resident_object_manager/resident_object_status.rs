const IS_IN_USE: u8 = 1 << 0;
const IS_MUTABLE_REF_ACTIVE: u8 = 1 << 1;
const ENABLE_PARTIAL_DIRTINESS_TRACKING: u8 = 1 << 2;
const DATA_DIRTY: u8 = 1 << 3;
const GENERAL_METADATA_DIRTY: u8 = 1 << 4;

/*
The bit usage is as follows:
|Bit|Usage|
0    Is In Use (any references currently open?)
1    Is Mutable Active (any mutable references currently open?)
2    Partial Dirtiness Tracking Enabled (indicates whether to use in place dirtiness tracking or not)
3    Is Data Dirty (also used as a cache if partial dirtiness tracking is enabled)
4    General Metadata Dirty (offset, layout)
5    [Unused]
6    [Unused]
7    [Unused]
*/

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ResidentObjectStatus {
    bit_list: u8,
}

macro_rules! generate_functions {
    ($bit: ident, $get_name: ident, $set_name: ident) => {
        #[inline]
        pub(crate) fn $get_name(&self) -> bool {
            self.is_set($bit)
        }

        #[inline]
        pub(crate) fn $set_name(&mut self, val: bool) {
            self.set($bit, val);
        }
    };
}

impl ResidentObjectStatus {
    /// The whole metadata is dirty except from the data
    pub(crate) const fn new_metadata_dirty(
        partial_dirtiness_tracking: bool,
    ) -> ResidentObjectStatus {
        let mut instance = ResidentObjectStatus {
            bit_list: GENERAL_METADATA_DIRTY,
        };

        if partial_dirtiness_tracking {
            instance.bit_list |= ENABLE_PARTIAL_DIRTINESS_TRACKING;
        }

        instance
    }

    #[inline]
    fn is_set(&self, bitmask: u8) -> bool {
        (self.bit_list & bitmask) != 0
    }

    #[inline]
    fn set(&mut self, bitmask: u8, state: bool) {
        if state {
            // set
            self.bit_list |= bitmask;
        } else {
            // unset
            self.bit_list &= !bitmask;
        }
    }

    generate_functions!(IS_IN_USE, is_in_use, set_is_in_use);
    generate_functions!(
        IS_MUTABLE_REF_ACTIVE,
        is_mutable_ref_active,
        set_is_mutable_ref_active
    );
    generate_functions!(DATA_DIRTY, is_data_dirty, set_data_dirty);
    generate_functions!(
        GENERAL_METADATA_DIRTY,
        is_general_metadata_dirty,
        set_general_metadata_dirty
    );
    generate_functions!(
        ENABLE_PARTIAL_DIRTINESS_TRACKING,
        is_partial_dirtiness_tracking_enabled,
        set_is_partial_dirtiness_tracking_enabled
    );
}

impl Default for ResidentObjectStatus {
    fn default() -> Self {
        Self::new_metadata_dirty(false)
    }
}
