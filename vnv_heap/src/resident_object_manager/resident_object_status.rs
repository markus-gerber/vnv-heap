/*
 *  Copyright (C) 2025  Markus Elias Gerber
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

const IS_IN_USE: u8 = 1 << 0;
const IS_MUTABLE_REF_ACTIVE: u8 = 1 << 1;
const ENABLE_PARTIAL_DIRTINESS_TRACKING: u8 = 1 << 2;
const DATA_DIRTY: u8 = 1 << 3;
const CLOCK_ACCESSED: u8 = 1 << 4;
const CLOCK_MODIFIED: u8 = 1 << 5;

/*
The bit usage is as follows:
|Bit|Usage|
0    Is In Use (any references currently open?)
1    Is Mutable Active (any mutable references currently open?)
2    Partial Dirtiness Tracking Enabled (indicates whether to use in place dirtiness tracking or not)
3    Is Data Dirty (also used as a cache if partial dirtiness tracking is enabled)
4    Clock status bit: was accessed (for more information look into ClockObjectManagementModule)
5    Clock status bit: was modified (for more information look into ClockObjectManagementModule)
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

        #[allow(unused)]
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
            bit_list: 0,
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
        ENABLE_PARTIAL_DIRTINESS_TRACKING,
        is_partial_dirtiness_tracking_enabled,
        set_is_partial_dirtiness_tracking_enabled
    );
    generate_functions!(
        CLOCK_ACCESSED,
        is_clock_accessed_bit_set,
        set_clock_accessed_bit
    );
    generate_functions!(
        CLOCK_MODIFIED,
        is_clock_modified_bit_set,
        set_clock_modified_bit
    );
}

impl Default for ResidentObjectStatus {
    fn default() -> Self {
        Self::new_metadata_dirty(false)
    }
}
