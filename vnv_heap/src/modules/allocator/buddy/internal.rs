// original code from https://github.com/rcore-os/buddy_system_allocator
// modifications: added allocate_at, made linked list sorted, and removed statistics

#![allow(dead_code)]

use core::alloc::Layout;
use core::cmp::{max, min};
use core::mem::size_of;
use core::ptr::NonNull;

pub struct Heap<const ORDER: usize> {
    // buddy system with max order of `ORDER`
    free_list: [super::linked_list::LinkedList; ORDER],
}

impl<const ORDER: usize> Heap<ORDER> {
    /// Create an empty heap
    pub const fn new() -> Self {
        Heap {
            free_list: [super::linked_list::LinkedList::new(); ORDER],
        }
    }

    /// Create an empty heap
    pub const fn empty() -> Self {
        Self::new()
    }

    /// Add a range of memory [start, end) to the heap
    pub unsafe fn add_to_heap(&mut self, mut start: usize, mut end: usize) {
        // avoid unaligned access on some platforms
        start = (start + size_of::<usize>() - 1) & (!size_of::<usize>() + 1);
        end &= !size_of::<usize>() + 1;
        assert!(start <= end);

        let mut current_start = start;

        while current_start + size_of::<usize>() <= end {
            let lowbit = current_start & (!current_start + 1);
            let size = min(lowbit, prev_power_of_two(end - current_start));

            self.free_list[size.trailing_zeros() as usize].push(current_start as *mut usize);
            current_start += size;
        }
    }

    /// Add a range of memory [start, start+size) to the heap
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.add_to_heap(start, start + size);
    }

    /// Alloc a range of memory from the heap satifying `layout` requirements
    pub fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, ()> {
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );
        let class = size.trailing_zeros() as usize;
        for i in class..self.free_list.len() {
            // Find the first non-empty size class
            if !self.free_list[i].is_empty() {
                // Split buffers
                for j in (class + 1..i + 1).rev() {
                    if let Some(block) = self.free_list[j].pop() {
                        unsafe {
                            self.free_list[j - 1]
                                .push((block as usize + (1 << (j - 1))) as *mut usize);
                            self.free_list[j - 1].push(block);
                        }
                    } else {
                        return Err(());
                    }
                }

                let result = NonNull::new(
                    self.free_list[class]
                        .pop()
                        .expect("current block should have free space now")
                        as *mut u8,
                );
                if let Some(result) = result {
                    return Ok(result);
                } else {
                    return Err(());
                }
            }
        }
        Err(())
    }

    /// ### Safety
    ///
    /// `ptr` has to be aligned properly (to the start of a bucket)!!!
    pub unsafe fn alloc_at(&mut self, layout: Layout, ptr: *mut u8) -> Result<(), ()> {
        let ptr = ptr as usize;
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );

        let class = size.trailing_zeros() as usize;
        let mut curr_class_size = size;

        for i in class..self.free_list.len() {
            // Find the item that is containing the pointer
            let mut iter = self.free_list[i].iter_mut();
            let item = iter.find(|item| {
                let start = item.value() as usize;
                let end = start + curr_class_size;
                start <= ptr && ptr < end
            });

            if let Some(item) = item {
                // item found, remove it and split if necessary
                let mut curr_block = item.pop();

                for j in (class..i).rev() {
                    curr_class_size = curr_class_size >> 1;

                    // split this item
                    let block1 = curr_block as usize + (1 << j);
                    let block2 = curr_block;

                    // check in which of these two block our ptr is
                    let (continue_block, push_block) =
                        if block1 <= ptr && ptr < (block1 + curr_class_size) {
                            // ptr is somewhere in block1
                            // continue work with block1 and push block2 to linked list
                            (block1 as *mut usize, block2)
                        } else {
                            // ptr is somewhere in block2
                            // continue work with block2 and push block1 to linked list
                            (block2, block1 as *mut usize)
                        };

                    self.free_list[j].push(push_block);
                    curr_block = continue_block;
                }

                // ptr should be aligned
                debug_assert_eq!(curr_block as usize, ptr);

                return Ok(());
            }

            // update size
            curr_class_size = curr_class_size << 1;
        }
        Err(())
    }

    /// Dealloc a range of memory from the heap
    pub fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );
        let class = size.trailing_zeros() as usize;

        unsafe {
            // Put back into free list
            self.free_list[class].push(ptr.as_ptr() as *mut usize);

            // Merge free buddy lists
            let mut current_ptr = ptr.as_ptr() as usize;
            let mut current_class = class;

            while current_class < self.free_list.len() - 1 {
                let buddy = current_ptr ^ (1 << current_class);
                let mut flag = false;
                for block in self.free_list[current_class].iter_mut() {
                    if block.value() as usize == buddy {
                        block.pop();
                        flag = true;
                        break;
                    }
                }

                // Free buddy found
                if flag {
                    self.free_list[current_class].pop();
                    current_ptr = min(current_ptr, buddy);
                    current_class += 1;
                    self.free_list[current_class].push(current_ptr as *mut usize);
                } else {
                    break;
                }
            }
        }
    }
}

pub(crate) fn prev_power_of_two(num: usize) -> usize {
    1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}

#[cfg(test)]
pub(super) mod test {
    use super::Heap;

    pub(crate) fn check_heap_integrity<const ORDER: usize>(
        heap1: &mut Heap<ORDER>,
        heap2: &mut Heap<ORDER>,
        diff: isize,
    ) {
        for i in 0..ORDER {
            super::super::linked_list::test::check_linked_list_integrity(
                &mut heap1.free_list[i],
                &mut heap2.free_list[i],
                diff,
            );
        }
    }
}
