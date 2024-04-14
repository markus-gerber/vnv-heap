// some code from: https://github.com/rcore-os/buddy_system_allocator

mod linked_list;

use std::{cmp::{max, min}, mem::size_of, ptr::{null, NonNull}};
use super::AllocatorModule;

pub struct BuddyAllocatorModule<const ORDER: usize> {
    /// Pointer to current start of heap
    /// 
    /// This is used to update all the pointer inside the free lists once this
    /// heap was unmapped and mapped again, because the address of this heap might/will change
    base_ptr: *const u8,

    /// buddy system with max order of `ORDER`
    free_list: [linked_list::LinkedList; ORDER],
}

impl<const ORDER: usize> AllocatorModule for BuddyAllocatorModule<ORDER> {
    fn new() -> Self {
        BuddyAllocatorModule {
            base_ptr: null(),
            free_list: [linked_list::LinkedList::new(); ORDER]
        }
    }

    unsafe fn init(&mut self, start: *mut u8, size: usize) -> usize {
        self.base_ptr = start;

        let mut start = start as usize;

        // add free memory to free list
        let mut end = start + size;

        // avoid unaligned access on some platforms
        start = (start + size_of::<usize>() - 1) & (!size_of::<usize>() + 1);
        end &= !size_of::<usize>() + 1;
        assert!(start <= end);

        let mut current_start = start;

        let mut max_bucket_size = 0;
        while current_start + size_of::<usize>() <= end {
            let lowbit = current_start & (!current_start + 1);
            let size = min(lowbit, prev_power_of_two(end - current_start));
            max_bucket_size = max(max_bucket_size, size);

            self.free_list[size.trailing_zeros() as usize].push(current_start as *mut usize);
            current_start += size;
        }
        
        max_bucket_size
    }

    unsafe fn allocate(&mut self, layout: &std::alloc::Layout, max_alloc_size: usize) -> Result<(NonNull<u8>, usize), ()> {
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
                    let mut curr_bucket = max_alloc_size.trailing_zeros() as usize;
                    while self.free_list[curr_bucket].is_empty() {
                        if curr_bucket == 0 {
                            break;
                        }

                        curr_bucket -= 1;
                    }

                    let res_max_alloc_size = if self.free_list[curr_bucket].is_empty() {
                        // this is the case where curr_bucket = 0 and this bucket is empty too
                        0
                    } else {
                        // free bucket found, set remaining size
                        1 << curr_bucket
                    };

                    return Ok((result, res_max_alloc_size));
                } else {
                    return Err(());
                }
            }
        }
        Err(())
    }

    unsafe fn deallocate(&mut self, ptr: std::ptr::NonNull<u8>, layout: &std::alloc::Layout, max_alloc_size: usize) -> usize {
        let size = max(
            layout.size().next_power_of_two(),
            max(layout.align(), size_of::<usize>()),
        );
        let class = size.trailing_zeros() as usize;
        let mut max_created_class = class;

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

                    // newly created class is already greater than previous max class
                    max_created_class = current_class;
                } else {
                    break;
                }
            }
        }

        let created_size = 1 << max_created_class;
        max(max_alloc_size, created_size)
    }
    /*
    fn get_max_alloc_size(&self) -> usize {
        let mut max_class: usize = 0;
        for i in 0..ORDER {
            if !self.free_list[i].is_empty() {
                max_class = i;
            }
        }

        if max_class == 0 && self.free_list[0].is_empty() {
            0
        } else {
            1 << max_class
        }
    }*/

    fn calc_min_size_for_layout(layout: &std::alloc::Layout) -> usize {
        max(layout.size(), size_of::<usize>()).next_power_of_two()
    }

    unsafe fn on_ptr_change(&mut self, old_base_ptr: *mut u8, new_base_ptr: *mut u8) {
        let offset = new_base_ptr.offset_from(old_base_ptr);
        for i in 0..ORDER {
            self.free_list[i].update_ptrs(offset);
        }
    }
}

fn prev_power_of_two(num: usize) -> usize {
    1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}
