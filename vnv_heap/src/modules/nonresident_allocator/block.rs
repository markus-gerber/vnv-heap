use std::mem::size_of;
use crate::util::div_ceil;
use super::NonResidentAllocatorModule;

type BitListType = usize;

pub const fn calc_non_resident_block_allocator_bit_list_size(
    block_size: usize,
    total_size: usize,
) -> usize {
    let bit_cnt = div_ceil(total_size, block_size);

    div_ceil(bit_cnt, size_of::<BitListType>())
}

pub struct NonResidentBlockAllocator<const BLOCK_SIZE: usize, const BIT_LIST_SIZE: usize> {
    bit_list: [BitListType; BIT_LIST_SIZE],
    offset: usize,
    size: usize
}

impl<const BLOCK_SIZE: usize, const BIT_LIST_SIZE: usize> NonResidentAllocatorModule
    for NonResidentBlockAllocator<BLOCK_SIZE, BIT_LIST_SIZE>
{
    fn new() -> Self {
        Self {
            bit_list: [0; BIT_LIST_SIZE],
            offset: 0,
            size: 0
        }
    }

    fn init<S: crate::modules::persistent_storage::PersistentStorageModule>(
        &mut self,
        offset: usize,
        size: usize,
        _storage_module: &mut S,
    ) -> Result<(), ()> {
        let min_bit_list_size = calc_non_resident_block_allocator_bit_list_size(BLOCK_SIZE, size);
        assert!(min_bit_list_size <= self.bit_list.len());

        for item in &mut self.bit_list {
            *item = 0;
        }

        // no need to check alignment as it is not needed on external storage 
        self.offset = offset;

        // size should be a multiple of BLOCK_SIZE
        let new_size = (size / BLOCK_SIZE) * BLOCK_SIZE;
        self.size = new_size;

        Ok(())
    }

    fn allocate<S: crate::modules::persistent_storage::PersistentStorageModule>(
        &mut self,
        layout: std::alloc::Layout,
        _storage_module: &mut S,
    ) -> Result<usize, ()> {
        // we don't need to worry about alignment on non volatile storage
        let size = layout.size();

        // calculate the amount of blocks needed for this layout
        let required_blocks = div_ceil(size, BLOCK_SIZE);

        let bit_list_size = Self::bit_list_available_size(self.size);

        let mut list_index_start = 0;
        let mut list_index_end = 0;
        let mut bit_index_start = 0;
        let mut bit_index_end = 0;

        let mut matched_block_cnt = 0;

        'outer: for list_index in 0..bit_list_size {
            let mut item = self.bit_list[list_index];

            let end = if list_index == bit_list_size - 1 {
                Self::bit_list_last_item_block_cnt(self.size)
            } else {
                size_of::<BitListType>()
            };

            for i in 0..end {
                if item & 1 == 0 {
                    matched_block_cnt += 1;

                    if matched_block_cnt == 1  {

                        // first free block
                        list_index_start = list_index;
                        bit_index_start = i;
                    }

                    if matched_block_cnt == required_blocks {
                        // enough blocks were found
                        list_index_end = list_index;
                        bit_index_end = i;
                        break 'outer;
                    }

                } else {
                    matched_block_cnt = 0;
                }

                item = item >> 1;
            }
        }

        if matched_block_cnt != required_blocks {
            return Err(());
        }

        // mark the whole block range that was found previously as used
        for list_index in list_index_start..=list_index_end {
            let bit_start = if list_index == list_index_start {
                bit_index_start
            } else {
                0
            };

            let bit_end = if list_index == list_index_end {
                bit_index_end
            } else {
                size_of::<BitListType>() - 1
            };

            for i in bit_start..=bit_end {
                let mask = 1 << i;
                self.bit_list[list_index] |= mask;
            }
        }

        let rel_offset = list_index_start * (size_of::<BitListType>() * BLOCK_SIZE) + bit_index_start * BLOCK_SIZE;
        Ok(self.offset + rel_offset)
    }

    fn deallocate<S: crate::modules::persistent_storage::PersistentStorageModule>(
        &mut self,
        offset: usize,
        layout: std::alloc::Layout,
        _storage_module: &mut S,
    ) -> Result<(), ()> {
        debug_assert_eq!((offset - self.offset) % BLOCK_SIZE, 0, "offset should be multiple of BLOCK_SIZE");

        // we don't need to worry about alignment on non volatile storage
        let size = layout.size();

        // calculate the amount of blocks that this layout required
        let mut required_blocks = div_ceil(size, BLOCK_SIZE);

        let rel_offset = offset - self.offset;
        let block_offset = rel_offset / BLOCK_SIZE;

        let start_item_index = block_offset / size_of::<BitListType>();
        let start_bit_index = block_offset % size_of::<BitListType>();

        'outer: for index in start_item_index.. {
            let start = if index == start_item_index {
                start_bit_index
            } else {
                0
            };

            for bit_index in start..size_of::<BitListType>() {
                let mask = 1 << bit_index;
                debug_assert_ne!(self.bit_list[index] & mask, 0, "Bit should be in use. Wrong dealloc? {:?}", self.bit_list);

                self.bit_list[index] &= !mask;

                required_blocks -= 1;
                if required_blocks == 0 {
                    break 'outer;
                }
            }
        }

        Ok(())
    }
}

impl<const BLOCK_SIZE: usize, const BIT_LIST_SIZE: usize> NonResidentBlockAllocator<BLOCK_SIZE, BIT_LIST_SIZE> {
    fn bit_list_available_size(size: usize) -> usize {
        div_ceil(size / BLOCK_SIZE, size_of::<BitListType>())
    }

    fn bit_list_last_item_block_cnt(size: usize) -> usize {
        let tmp = (size / BLOCK_SIZE) % size_of::<BitListType>();
        if tmp == 0 {
            size_of::<BitListType>()
        } else {
            tmp
        }
    }
}

#[cfg(test)]
mod test {
    use std::mem::size_of;

    use crate::{modules::{
        nonresident_allocator::test::{
            test_non_resident_allocator_simple_generic, AllocatedRegion,
        },
        persistent_storage::PersistentStorageModule,
    }, util::round_up_to_nearest};

    use super::{BitListType, NonResidentBlockAllocator};

    const BLOCK_SIZE: usize = 32;

    #[test]
    pub(super) fn test_non_resident_allocator_simple() {
        test_non_resident_allocator_simple_generic(check_integrity, "test_non_resident_allocator_simple_block");
    }

    fn check_integrity<S: PersistentStorageModule>(
        regions: &Vec<AllocatedRegion>,
        allocator: &NonResidentBlockAllocator<BLOCK_SIZE, 64>,
        _storage: &mut S,
    ) {
        let allocator_offset = 0;

        let items: Vec<AllocatedRegion> = regions.iter().map(|x| x.clone()).collect();

        let mut curr_offset = allocator_offset;
        for list_index in 0..allocator.bit_list.len() {
            let item = allocator.bit_list[list_index];

            for i in 0..size_of::<BitListType>() {
                let found = items.iter().any(|x| {
                    x.offset <= curr_offset && curr_offset < (x.offset + round_up_to_nearest(x.size, BLOCK_SIZE))
                });

                if found {
                    assert!((item & (1 << i)) != 0);
                } else {
                    assert!((item & (1 << i)) == 0);    
                }

                curr_offset += BLOCK_SIZE;
            }
        }


    }
}
