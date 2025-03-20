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

use core::{
    alloc::Layout,
    mem::{align_of, size_of},
    ops::Range,
    ptr::slice_from_raw_parts_mut,
    u8,
};

use super::ResidentObjectMetadata;
use crate::util::div_ceil;
use static_assertions::{const_assert, const_assert_eq};

/// This value indicates how big the blocks should be on which dirtiness is tracked
/// You surely can change the value, but lower block sizes come with higher metadata costs (in RAM and on NV storage)
pub(crate) const PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE: usize = {
    const SIZE: usize = size_of::<usize>() * 8;

    // size check
    const_assert!(SIZE < SizeInfoCache::max_block_size());

    SIZE
};

/// How big can an object be at a maximum?
pub(crate) const MAX_SUPPORTED_PARTIAL_DIRTY_OBJ_SIZE: usize = {
    // constraint: byte_offset (which is an u8) should not overflow
    // this will implicitly guarantee that byte_count does not overflow too (as always: byte_count <= byte_offset)
    const MAX_SIZE: usize =
        PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 8 * MAX_SUPPORTED_PARTIAL_DIRTY_BUF_SIZE;
    const_assert_eq!(MAX_SIZE % align_of::<ResidentObjectMetadata>(), 0);

    MAX_SIZE
};

/// How long can the `bit_list` be (in bytes)? This is limited because of `PartialDirtinessTrackingInfo::byte_count`
pub(crate) const MAX_SUPPORTED_PARTIAL_DIRTY_BUF_SIZE: usize = u8::MAX as usize;

/// Information on partial dirtiness tracking
#[derive(Clone, Copy, PartialEq)]
#[repr(packed)]
pub(crate) struct PartialDirtinessTrackingInfo {
    /// Amount of bytes currently used by partial dirtiness tracking
    /// This is also the offset where to find the partial dirtiness list based
    /// on the starting address of the resident object metadata
    pub(crate) byte_count: u8,

    size_info_cache: SizeInfoCache,
}

impl PartialDirtinessTrackingInfo {
    pub(crate) const fn new_unused() -> Self {
        Self {
            byte_count: 0,
            size_info_cache: SizeInfoCache::new_unused(),
        }
    }

    pub(crate) fn new_used<T>() -> Self {
        let layout = Layout::new::<T>();
        Self::new_used_dynamic(&layout)
    }

    pub(crate) const fn calc_bit_and_byte_count(data_size: usize) -> (usize, usize) {
        debug_assert!(
            data_size <= MAX_SUPPORTED_PARTIAL_DIRTY_OBJ_SIZE,
            "Object has to be in the size limit!"
        );

        // count = ceil(size / BLOCK_SIZE)
        let bit_count = div_ceil(data_size, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE);
        let byte_count = div_ceil(bit_count, 8);

        (bit_count, byte_count)
    }

    pub(crate) fn new_used_dynamic(data_layout: &Layout) -> Self {
        // this should also be caught at another stage
        debug_assert!(
            data_layout.size() <= MAX_SUPPORTED_PARTIAL_DIRTY_OBJ_SIZE,
            "Object has to be in the size limit!"
        );

        // count = ceil(size / BLOCK_SIZE)
        let (bit_count, byte_count) = Self::calc_bit_and_byte_count(data_layout.size());

        let mut last_block_size = data_layout.size() % PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE;
        if last_block_size == 0 {
            last_block_size = PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE;
        }
        let mut last_byte_bit_cnt = bit_count % 8;
        if last_byte_bit_cnt == 0 {
            last_byte_bit_cnt = 8;
        }

        Self {
            byte_count: byte_count as u8,
            size_info_cache: SizeInfoCache::new(last_byte_bit_cnt, last_block_size),
        }
    }

    pub(crate) fn get_dirty_buf_slice<'a>(
        &'a self,
        base_ptr: *const ResidentObjectMetadata,
    ) -> &'a mut [u8] {
        let base_ptr = (base_ptr as *const u8) as usize - (self.byte_count as usize);

        let slice_ptr = slice_from_raw_parts_mut(base_ptr as *mut u8, self.byte_count as usize);
        let slice = unsafe { slice_ptr.as_mut().unwrap() };

        slice
    }

    /// **Safety**: Make sure no two wrappers exist at the same time
    pub(crate) unsafe fn get_wrapper<'a>(
        &'a self,
        base_ptr: *const ResidentObjectMetadata,
    ) -> PartialDirtinessTrackingWrapper<'a> {
        let size_info_cache = self.size_info_cache;

        PartialDirtinessTrackingWrapper {
            data_range: self.get_dirty_buf_slice(base_ptr),
            size_info_cache,
        }
    }
}

pub(crate) struct PartialDirtinessTrackingWrapper<'a> {
    /// Bitlist indicating which parts are dirty and which not
    /// Looks like this (- means unused): [01010110][00001111][---01011]
    /// (So bytes are always filled from the right, but the list starts with the most left byte)
    data_range: &'a mut [u8],

    size_info_cache: SizeInfoCache,
}

impl<'a> PartialDirtinessTrackingWrapper<'a> {
    pub(crate) fn get_dirty_size(&self) -> usize {
        self.calc_dirty_size()
    }

    fn calc_dirty_size(&self) -> usize {
        let mut dirty_size = 0;
        for byte_index in 0..self.data_range.len() {
            let mut cpy = self.data_range[byte_index];

            // could be more efficient with a lookup table
            for bit_index in 0..8 {
                if cpy & 0x1 == 1 {
                    if byte_index == self.data_range.len() - 1
                        && bit_index == (self.size_info_cache.get_last_byte_bit_cnt() as usize) - 1
                    {
                        dirty_size += self.size_info_cache.get_last_block_size() as usize
                    } else {
                        dirty_size += PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                    }
                }
                cpy = cpy >> 1;
            }
        }

        dirty_size
    }

    pub(crate) fn set_all_blocks_synced(&mut self) {
        // reset is most performant
        self.reset();
    }

    pub(crate) fn reset(&mut self) {
        self.data_range.fill(0);
    }

    pub(crate) fn reset_and_set_all_blocks_dirty(&mut self) {
        if self.data_range.len() == 0 {
            return;
        }

        // first, set all bytes dirty that certainly are used completely
        for index in 0..(self.data_range.len() - 1) {
            self.data_range[index] = u8::MAX;
        }

        // now we want to process the last byte
        // however, we are required by the implementation only to set the bits that are used by the dirtiness tracking
        if self.size_info_cache.get_last_byte_bit_cnt() == 8 {
            // yay, all of the bits in this byte are used
            // so we can just flip them
            self.data_range[self.data_range.len() - 1] = u8::MAX;
        } else {
            // not all of the bits in this byte are used
            // use bitmask
            self.data_range[self.data_range.len() - 1] =
                (1 << self.size_info_cache.get_last_byte_bit_cnt()) - 1
        }
    }

    #[allow(unused)]
    pub(crate) fn set_all_blocks_dirty(&mut self) {
        self.reset_and_set_all_blocks_dirty();
    }

    #[allow(unused)]
    pub(crate) fn get_dirty_size_in_range(
        &self,
        addr_offset: usize,
        size: usize,
    ) -> usize {
        self.get_dirty_status_in_range(addr_offset, size, true)
    }

    pub(crate) fn get_non_dirty_size_in_range(
        &self,
        addr_offset: usize,
        size: usize,
    ) -> usize {
        self.get_dirty_status_in_range(addr_offset, size, false)
    }

    #[inline]
    pub(crate) fn get_dirty_status_in_range(
        &self,
        addr_offset: usize,
        size: usize,
        dirty: bool
    ) -> usize {
        let mut res = 0;
        for i in 0..div_ceil(size, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE) {
            res += self.check_dirty_status_in_block(
                addr_offset + i * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE,
                dirty,
            );
        }
        res
    }

    #[inline]
    fn check_dirty_status_in_block(&self, addr_offset: usize, dirty: bool) -> usize {
        let block_index = addr_offset / PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE;
        let byte_index = block_index / 8;
        let bit_index = block_index % 8;

        debug_assert!(byte_index < self.data_range.len());

        #[cfg(debug_assertions)]
        if byte_index == self.data_range.len() - 1 {
            debug_assert!(bit_index < self.size_info_cache.get_last_byte_bit_cnt() as usize);
        }

        let item_ref = &self.data_range[byte_index];
        let bit_mask = 1 << (bit_index);
        let is_dirty = (*item_ref & bit_mask) != 0;
        
        if !is_dirty && dirty {
            return 0;
        } else if is_dirty && !dirty {
            return 0;
        }

        if byte_index == self.data_range.len() - 1
            && bit_index == (self.size_info_cache.get_last_byte_bit_cnt() as usize) - 1
        {
            self.size_info_cache.get_last_block_size() as usize
        } else {
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
        }
    }


    pub(crate) fn set_range_dirty(
        &mut self,
        addr_offset: usize,
        size: usize,
    ) {
        self.update_range_dirty(addr_offset, size, true);
    }

    #[allow(unused)]
    pub(crate) fn unset_range_dirty(
        &mut self,
        addr_offset: usize,
        size: usize,
    ) {
        self.update_range_dirty(addr_offset, size, false);
    }

    #[inline]
    fn update_range_dirty(
        &mut self,
        addr_offset: usize,
        size: usize,
        dirty: bool,
    ) {
        for i in 0..div_ceil(size, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE) {
            self.update_block_dirty(
                addr_offset + i * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE,
                dirty,
            )
        }
    }

    #[inline]
    fn update_block_dirty(&mut self, addr_offset: usize, dirty: bool) {
        let block_index = addr_offset / PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE;
        let byte_index = block_index / 8;
        let bit_index = block_index % 8;

        debug_assert!(byte_index < self.data_range.len());

        #[cfg(debug_assertions)]
        if byte_index == self.data_range.len() - 1 {
            debug_assert!(bit_index < self.size_info_cache.get_last_byte_bit_cnt() as usize);
        }

        // find and update the item
        let item_ref = &mut self.data_range[byte_index];
        let bit_mask = 1 << bit_index;

        if dirty {
            *item_ref |= bit_mask;
        } else {
            *item_ref &= !bit_mask;
        }
    }

    /// Returns an iterator over all dirty block ranges
    pub(crate) fn dirty_iter<'b>(&'b mut self) -> SyncDirtyIter<'b, 'a> {
        SyncDirtyIter {
            curr_byte: self.data_range[0],
            wrapper_data: self,
            curr_bit_id: 0,
            curr_byte_id: 0,
            reached_end: false,
        }
    }
}

pub(crate) struct SyncDirtyIter<'a, 'b> {
    curr_byte: u8,
    curr_byte_id: usize,
    curr_bit_id: u8,
    reached_end: bool,
    wrapper_data: &'a mut PartialDirtinessTrackingWrapper<'b>,
}

impl SyncDirtyIter<'_, '_> {
    pub(crate) fn next(&mut self) -> Option<Range<usize>> {
        if self.reached_end {
            return None;
        }

        // step 1: find first bit that is set

        let start = loop {
            if self.reached_end {
                // we did not find any 1
                return None;
            }

            // as the unused bits are required to always be 0
            // we don't need any if cases here
            let bit = self.next_bit();
            if bit == 1 {
                // yay we found a bit that is set!

                /*
                if self.update_status {
                    // unset current bit
                    self.wrapper_data.data_range[self.curr_byte_id] &= !(1 << self.curr_bit_id);
                }*/

                let start = self.calc_curr_offset();
                self.advance_bit();
                break start;
            }

            self.advance_bit();
        };

        // step 2: loop until there are no 1s anymore
        while !self.reached_end {
            let bit = self.next_bit();
            if bit == 0 {
                break;
            }

            /*
            if self.update_status {
                // unset current bit
                self.wrapper_data.data_range[self.curr_byte_id] &= !(1 << self.curr_bit_id);
            }*/

            self.advance_bit();
        }

        // step 3: no calculate the end offset
        let end = if self.reached_end {
            // we reached the end
            // we have to be careful, maybe the size of the last block is not PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
            let mut offset = (self.curr_byte_id * 8 + (self.curr_bit_id as usize) - 1)
                * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE;

            // last block size
            offset += self.wrapper_data.size_info_cache.get_last_block_size() as usize;

            offset
        } else {
            // this is not the last bit
            // the size of this block is equal to PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
            self.calc_curr_offset()
        };

        return Some(start..end);
    }

    #[inline]
    fn calc_curr_offset(&self) -> usize {
        (self.curr_byte_id * 8 + (self.curr_bit_id as usize)) * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
    }

    #[inline]
    fn next_bit(&mut self) -> u8 {
        let bit: u8 = self.curr_byte & 1;
        bit
    }

    #[inline]
    fn advance_bit(&mut self) {
        self.curr_bit_id = (self.curr_bit_id + 1) % 8;
        if self.curr_bit_id != 0 {
            // advance bit

            if self.curr_byte_id == self.wrapper_data.data_range.len() - 1
                && self.curr_bit_id
                    >= self.wrapper_data.size_info_cache.get_last_byte_bit_cnt() as u8
            {
                self.reached_end = true;
            } else {
                self.curr_byte = self.curr_byte >> 1;
            }
        } else {
            // advance byte
            self.curr_byte_id += 1;

            // check if we reached the end
            if self.curr_byte_id >= self.wrapper_data.data_range.len() {
                self.reached_end = true;
            } else {
                // update byte
                self.curr_byte = self.wrapper_data.data_range[self.curr_byte_id];
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct SizeInfoCache {
    data: u16,
}

impl SizeInfoCache {
    const LAST_BYTE_BIT_CNT_OFFSET: u16 = 0;
    const LAST_BLOCK_SIZE_OFFSET: u16 = 3;
    const GET_LAST_BYTE_BIT_CNT_BITMASK: u16 = ((1 << Self::LAST_BLOCK_SIZE_OFFSET) - 1);

    const fn new_unused() -> Self {
        Self { data: 0 }
    }

    fn new(mut last_byte_bit_cnt: usize, last_block_size: usize) -> Self {
        debug_assert!(last_byte_bit_cnt > 0);
        last_byte_bit_cnt -= 1;

        // size checks
        debug_assert!(last_byte_bit_cnt < (1 << Self::LAST_BLOCK_SIZE_OFFSET));
        debug_assert!(last_block_size < Self::max_block_size());

        // cast to right types
        let last_byte_bit_cnt = last_byte_bit_cnt as u16;
        let last_block_size = last_block_size as u16;

        Self {
            data: last_byte_bit_cnt | last_block_size << Self::LAST_BLOCK_SIZE_OFFSET,
        }
    }

    fn get_last_byte_bit_cnt(&self) -> u16 {
        debug_assert_eq!(Self::LAST_BYTE_BIT_CNT_OFFSET, 0);
        (self.data & Self::GET_LAST_BYTE_BIT_CNT_BITMASK) + 1
    }

    fn get_last_block_size(&self) -> u16 {
        self.data >> Self::LAST_BLOCK_SIZE_OFFSET
    }

    const fn max_block_size() -> usize {
        1 << (16 - Self::LAST_BLOCK_SIZE_OFFSET)
    }
}

#[cfg(test)]
mod test {
    use super::{
        PartialDirtinessTrackingWrapper, SizeInfoCache, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE,
    };
    /*
        #[test]
        fn test_partial_dirtiness_wrapper_iter() {
            let mut data_range = [0u8; 3];
            let mut obj = PartialDirtinessTrackingWrapper {
                data_range: &mut data_range,
                size_info_cache: SizeInfoCache::new(3, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2),
            };

            obj.update_block_dirty(
                PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            );
            obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 7 + 0);
            obj.update_block_dirty(
                PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 8 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 4,
            );
            obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + 0);
            obj.update_block_dirty(
                PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 4 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            );
            obj.update_block_dirty(
                PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 17 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 8,
            );
            obj.update_block_dirty(
                PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 18 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 8,
            );
            obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 15);
            obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 12);
            obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 11);
            obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 13);

            // bytes should look like this
            // [0b10011000, 0b10011001, 0b00000110]
            assert_eq!(obj.data_range, [0b10011000, 0b10111001, 0b00000110]);
            let mut iter = obj.sync_dirty_iter(true);
            assert_eq!(
                iter.next(),
                Some(
                    3 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE..5 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                )
            );
            assert_eq!(
                iter.wrapper_data.data_range,
                [0b10000000, 0b10111001, 0b00000110]
            );

            assert_eq!(
                iter.next(),
                Some(
                    7 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE..9 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                )
            );
            assert_eq!(
                iter.wrapper_data.data_range,
                [0b00000000, 0b10111000, 0b00000110]
            );

            assert_eq!(
                iter.next(),
                Some(
                    11 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                        ..14 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                )
            );
            assert_eq!(
                iter.wrapper_data.data_range,
                [0b00000000, 0b10000000, 0b00000110]
            );

            assert_eq!(
                iter.next(),
                Some(
                    15 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                        ..16 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                )
            );
            assert_eq!(
                iter.wrapper_data.data_range,
                [0b00000000, 0b00000000, 0b00000110]
            );

            let end =
                (18 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE) + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2;
            assert_eq!(
                iter.next(),
                Some(17 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE..end)
            );
            assert_eq!(
                iter.wrapper_data.data_range,
                [0b00000000, 0b00000000, 0b00000000]
            );

            assert_eq!(iter.next(), None);
        }
    */

    #[test]
    fn test_partial_dirtiness_wrapper_iter_no_update() {
        let mut data_range = [0u8; 3];
        let mut obj = PartialDirtinessTrackingWrapper {
            data_range: &mut data_range,
            size_info_cache: SizeInfoCache::new(3, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2),
        };

        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 7 + 0, true);
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 8 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 4,
            true,
        );
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + 0, true);
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 4 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 17 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 8,
            true,
        );
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 18 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 8,
            true,
        );
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 15, true);
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 12, true);
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 11, true);
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 13, true);

        // bytes should look like this
        // [0b10011000, 0b10011001, 0b00000110]
        assert_eq!(obj.data_range, [0b10011000, 0b10111001, 0b00000110]);
        let mut iter = obj.dirty_iter();
        assert_eq!(
            iter.next(),
            Some(
                3 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE..5 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
            )
        );
        assert_eq!(
            iter.wrapper_data.data_range,
            [0b10011000, 0b10111001, 0b00000110]
        );

        assert_eq!(
            iter.next(),
            Some(
                7 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE..9 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
            )
        );
        assert_eq!(
            iter.wrapper_data.data_range,
            [0b10011000, 0b10111001, 0b00000110]
        );

        assert_eq!(
            iter.next(),
            Some(
                11 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                    ..14 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
            )
        );
        assert_eq!(
            iter.wrapper_data.data_range,
            [0b10011000, 0b10111001, 0b00000110]
        );

        assert_eq!(
            iter.next(),
            Some(
                15 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
                    ..16 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE
            )
        );
        assert_eq!(
            iter.wrapper_data.data_range,
            [0b10011000, 0b10111001, 0b00000110]
        );

        let end =
            (18 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE) + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2;
        assert_eq!(
            iter.next(),
            Some(17 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE..end)
        );
        assert_eq!(
            iter.wrapper_data.data_range,
            [0b10011000, 0b10111001, 0b00000110]
        );

        assert_eq!(iter.next(), None);

        obj.set_all_blocks_synced();
        assert_eq!(obj.data_range, [0, 0, 0]);
    }

    #[test]
    fn test_partial_dirtiness_wrapper_update_range_dirty() {
        let mut data_range = [0u8; 3];
        let mut obj = PartialDirtinessTrackingWrapper {
            data_range: &mut data_range,
            size_info_cache: SizeInfoCache::new(3, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE),
        };

        obj.update_range_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );

        assert_eq!(obj.data_range, [0b00001000, 0, 0]);

        obj.update_range_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 7,
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );

        assert_eq!(obj.data_range, [0b10001000, 0, 0]);

        obj.update_range_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 6,
            4 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );

        assert_eq!(obj.data_range, [0b11001000, 0b00000111, 0]);

        obj.update_range_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 6,
            4 * PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE + (PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE - 1),
            true,
        );

        assert_eq!(obj.data_range, [0b11001000, 0b00000111, 0]);

        obj.update_range_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 7, 1, true);

        assert_eq!(obj.data_range, [0b11001000, 0b00000111, 0]);
    }

    #[test]
    fn test_partial_dirtiness_wrapper_update_block_dirty() {
        let mut data_range = [0u8; 3];
        let mut obj = PartialDirtinessTrackingWrapper {
            data_range: &mut data_range,
            size_info_cache: SizeInfoCache::new(3, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE),
        };

        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 7 + 0, true);
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 8 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 4,
            true,
        );
        obj.update_block_dirty(PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 3 + 0, true);
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 4 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 2,
            true,
        );
        obj.update_block_dirty(
            PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE * 17 + PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE / 8,
            true,
        );

        drop(obj);
        assert_eq!(
            data_range,
            [(1 << 3) | (1 << 4) | (1 << 7), (1 << 0), (1 << 1)]
        )
    }

    #[test]
    fn test_partial_dirtiness_wrapper_set_all_dirty() {
        let mut data_range = [0u8; 3];
        let mut obj = PartialDirtinessTrackingWrapper {
            data_range: &mut data_range,
            size_info_cache: SizeInfoCache::new(3, PARTIAL_DIRTINESS_TACKING_BLOCK_SIZE),
        };

        obj.set_all_blocks_dirty();
        drop(obj);
        assert_eq!(data_range, [0xff, 0xff, 0x7])
    }

    #[test]
    fn test_size_info_cache() {
        for bit_cnt in 0..(1 << 3) {
            for block_size in 1..(1 << 13) {
                test_size_info_cache_impl(bit_cnt + 1, block_size);
            }
        }
    }

    fn test_size_info_cache_impl(last_byte_bit_cnt: usize, last_block_size: usize) {
        let obj = SizeInfoCache::new(last_byte_bit_cnt, last_block_size);
        assert_eq!(
            obj.get_last_byte_bit_cnt() as usize,
            last_byte_bit_cnt,
            "data: {}",
            obj.data
        );
        assert_eq!(
            obj.get_last_block_size() as usize,
            last_block_size,
            "data: {}",
            obj.data
        );
    }
}
