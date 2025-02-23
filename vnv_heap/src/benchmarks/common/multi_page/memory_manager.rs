// this file contains the model for the baseline benchmark

use crate::util::div_ceil;

use super::{object::Object, AllocatorModule, PersistentStorageModule};
use core::{alloc::Layout, cell::RefCell, mem::size_of, ptr::NonNull};
use std::{cell::RefMut, ops::Range};

pub(crate) struct MemoryManager<
    'a,
    const PAGE_SIZE: usize,
    const PAGE_COUNT: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    inner: RefCell<MemoryManagerInner<'a, PAGE_SIZE, PAGE_COUNT, A, S>>,
}

impl<
        'a,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > MemoryManager<'a, PAGE_SIZE, PAGE_COUNT, A, S>
{
    pub(crate) fn new(storage: &'a mut S, alloc: A, max_dirty_pages: usize, pages: &'a mut [[u8; PAGE_SIZE]; PAGE_COUNT]) -> Self {
        Self {
            inner: RefCell::new(MemoryManagerInner::new(storage, alloc, max_dirty_pages, pages)),
        }
    }

    /// Allocates a new object.
    ///
    /// **Note**: The current implementation for this function is limited and should not be used for benchmarking
    /// as it does not 100% correctly track which pages are modified by the allocator (and thus does not flush them).
    #[allow(unused)]
    pub(crate) fn allocate<'b, T>(
        &'b self,
        data: T,
    ) -> Result<Object<'b, 'a, T, PAGE_SIZE, PAGE_COUNT, A, S>, ()>
    where
        'a: 'b,
    {
        let ptr = self.inner.borrow_mut().allocate(data)?;

        Ok(Object::new(ptr, &self.inner))
    }

    pub(crate) fn get_inner(
        &self,
    ) -> RefMut<'_, MemoryManagerInner<'a, PAGE_SIZE, PAGE_COUNT, A, S>> {
        self.inner.borrow_mut()
    }
}

pub(crate) const fn multi_page_calc_metadata_size<const PAGE_SIZE: usize, const PAGE_COUNT: usize, A: AllocatorModule, S: PersistentStorageModule>() -> usize {
    size_of::<MemoryManagerInner<PAGE_SIZE, PAGE_COUNT, A, S>>() + size_of::<S>()
}

pub(crate) struct MemoryManagerInner<
    'a,
    const PAGE_SIZE: usize,
    const PAGE_COUNT: usize,
    A: AllocatorModule,
    S: PersistentStorageModule,
> {
    pages: &'a mut [[u8; PAGE_SIZE]; PAGE_COUNT],
    open_references: [usize; PAGE_COUNT],
    modified_pages: [bool; PAGE_COUNT],
    modified_page_count: usize,
    modified_clock: PageClock<PAGE_COUNT>,
    modified_page_limit: usize,

    // just a dummy to calculate the amount of metadata needed
    #[allow(unused)]
    page_resident: [bool; PAGE_COUNT],

    storage: &'a mut S,
    allocator: A,
}

impl<
        'a,
        const PAGE_SIZE: usize,
        const PAGE_COUNT: usize,
        A: AllocatorModule,
        S: PersistentStorageModule,
    > MemoryManagerInner<'a, PAGE_SIZE, PAGE_COUNT, A, S>
{
    pub(crate) fn new(storage: &'a mut S, mut allocator: A, modified_page_limit: usize, pages: &'a mut [[u8; PAGE_SIZE]; PAGE_COUNT]) -> Self {
        assert_eq!(PAGE_SIZE % size_of::<usize>(), 0, "Page size ({}) must be a multiple of usize", PAGE_SIZE);
        assert!(size_of::<usize>() <= PAGE_SIZE, "{}", PAGE_SIZE);

        unsafe {
            allocator.reset();
            allocator.init((&mut pages[0]) as *mut _, PAGE_SIZE * PAGE_COUNT);
        };

        Self {
            pages,
            open_references: [0; PAGE_COUNT],

            storage,
            allocator,
            modified_page_limit,
            modified_clock: PageClock::new(),
            modified_page_count: 0,
            modified_pages: [false; PAGE_COUNT],
            page_resident: [false; PAGE_COUNT],
        }
    }

    #[allow(unused)]
    pub(crate) fn allocator(&mut self) -> &mut A {
        &mut self.allocator
    }

    pub(crate) fn allocate<T>(&mut self, data: T) -> Result<*mut T, ()> {
        let ptr = unsafe { self.allocator.allocate(Layout::new::<T>())? };
        let ptr = (ptr.as_ptr()) as *mut T;

        let pages = self.get_pages_for_obj(ptr as *mut u8, size_of::<T>());
        match self.make_pages_dirty(pages) {
            Ok(_) => {}
            Err(_) => {
                unsafe {
                    self.allocator
                        .deallocate(NonNull::new(ptr as *mut u8).unwrap(), Layout::new::<T>());
                }
                return Err(());
            }
        }

        unsafe { ptr.write(data) };
        Ok(ptr)
    }

    pub(crate) fn drop_and_deallocate<T>(&mut self, ptr: *mut T) {
        unsafe {
            ptr.drop_in_place();
            self.allocator
                .deallocate(NonNull::new(ptr as *mut u8).unwrap(), Layout::new::<T>());
        }

        // try to make pages dirty
        let pages = self.get_pages_for_obj(ptr as *mut u8, size_of::<T>());
        let _ = self.make_pages_dirty(pages);
    }

    pub(crate) fn acquire_mut<T>(&mut self, ptr: *mut T) -> Result<(), ()> {
        let pages = self.get_pages_for_obj(ptr as *mut u8, size_of::<T>());

        for page in pages.clone() {
            self.open_references[page] += 1;
        }

        // its better to first increment the open_references
        // because pages these pages are in use then and we wont flush them by accident
        match self.make_pages_dirty(pages.clone()) {
            Ok(_) => {}
            Err(_) => {
                // restore previous state
                for page in pages {
                    self.open_references[page] -= 1;
                }
                return Err(());
            }
        }

        Ok(())
    }

    pub(crate) fn release_mut<T>(&mut self, ptr: *mut T) {
        let pages = self.get_pages_for_obj(ptr as *mut u8, size_of::<T>());
        for page in pages {
            debug_assert!(self.open_references[page] > 0);
            debug_assert!(self.modified_pages[page]);

            self.open_references[page] -= 1;
        }
    }

    pub(crate) fn flush<T>(&mut self, ptr: *mut T) -> Result<(), ()> {
        let pages = self.get_pages_for_obj(ptr as *mut u8, size_of::<T>());
        for page in pages {
            self.flush_page(page)?;
        }

        Ok(())
    }

    fn make_pages_dirty(&mut self, pages: Range<usize>) -> Result<(), ()> {
        for page in pages {
            if self.modified_pages[page] {
                // already dirty
                self.modified_clock.access(page);
            } else {
                if self.modified_page_limit == self.modified_page_count {
                    // flush page
                    let to_flush = self
                        .modified_clock
                        .next(&mut self.open_references, &mut self.modified_pages)
                        .unwrap();
                    self.flush_page(to_flush)?;
                }

                // make page valid
                self.modified_clock.access(page);
                self.modified_pages[page] = true;
                self.modified_page_count += 1;

                self.check_integrity();
            }
        }

        Ok(())
    }

    fn flush_page(&mut self, page: usize) -> Result<(), ()> {
        if self.modified_pages[page] {
            // page is dirty
            self.storage.write(page * PAGE_SIZE, &self.pages[page])?;
            self.modified_pages[page] = false;
            self.modified_page_count -= 1;
        }

        self.check_integrity();

        Ok(())
    }

    fn get_pages_for_obj(&self, ptr: *mut u8, size: usize) -> Range<usize> {
        let offset = (ptr as usize) - ((&self.pages[0] as *const u8) as usize);
        let start = offset / PAGE_SIZE;
        let end = div_ceil(offset + size, PAGE_SIZE);
        start..end
    }

    fn check_integrity(&self) {
        debug_assert_eq!(
            self.modified_pages.iter().filter(|&&v| v).count(),
            self.modified_page_count
        );
    }
}

struct PageClock<const PAGE_COUNT: usize> {
    accessed: [bool; PAGE_COUNT],
    curr_page: usize,
}

impl<const PAGE_COUNT: usize> PageClock<PAGE_COUNT> {
    fn new() -> Self {
        Self {
            accessed: [false; PAGE_COUNT],
            curr_page: 0,
        }
    }

    fn access(&mut self, page: usize) {
        self.accessed[page] = true;
    }

    fn next(&mut self, open_references: &mut [usize], valid: &mut [bool]) -> Option<usize> {
        let start_page = self.curr_page;
        let mut iterations = 0;

        loop {
            // only consider pages that not used currently and are valid
            if open_references[self.curr_page] == 0 && valid[self.curr_page] {
                if !self.accessed[self.curr_page] {
                    // page lost its chance, choose it
                    let page = self.curr_page;

                    // update pointer before returning
                    self.curr_page = (self.curr_page + 1) % PAGE_COUNT;
                    return Some(page);
                } else {
                    // page was accessed, give it another chance
                    self.accessed[self.curr_page] = false;
                }
            }

            self.curr_page = (self.curr_page + 1) % PAGE_COUNT;

            if start_page == self.curr_page {
                iterations += 1;
                if iterations == 2 {
                    // we could not find a suitable page
                    return None;
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    #[allow(unused)]
    fn debug(&self, valid: &[bool]) {
        print!("ptr: {}, accessed: ", self.curr_page);
        for i in valid.iter().enumerate().filter(|(_, &v)| v).map(|(i, _)| i) {
            print!("{}->{} ", i, self.accessed[i]);
        }
        println!();
    }
}

#[cfg(test)]
mod test {
    use super::PageClock;

    #[test]
    fn test_page_clock() {
        struct TestWrapper<const PAGE_COUNT: usize> {
            clock: PageClock<PAGE_COUNT>,
            valid: [bool; PAGE_COUNT],
            open_refs: [usize; PAGE_COUNT],
            max_valid: usize,
        }

        impl<const PAGE_COUNT: usize> TestWrapper<PAGE_COUNT> {
            fn new(max_valid: usize) -> Self {
                Self {
                    clock: PageClock::new(),
                    valid: [false; PAGE_COUNT],
                    open_refs: [0; PAGE_COUNT],
                    max_valid,
                }
            }

            fn count_valid(&self) -> usize {
                self.valid.iter().filter(|&&v| v).count()
            }

            fn access(&mut self, page: usize) {
                if self.valid[page] {
                    // already valid
                    self.clock.access(page);
                } else {
                    if self.count_valid() == self.max_valid {
                        // no more valid pages
                        let page_to_evict = self
                            .clock
                            .next(&mut self.open_refs, &mut self.valid)
                            .unwrap();
                        self.valid[page_to_evict] = false;
                    }

                    // make page valid
                    self.clock.access(page);
                    self.valid[page] = true;
                }

                print!("{} -> ", page);
                self.clock.debug(&self.valid);
                self.debug();
            }

            fn debug(&self) {
                println!(
                    "Valid: {:?}",
                    self.valid
                        .iter()
                        .enumerate()
                        .filter(|(_, &v)| v)
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>()
                );
            }
        }

        let mut test = TestWrapper::<20>::new(4);
        test.valid[0] = true;
        test.open_refs[0] = 1;
        test.access(1);
        test.access(2);
        test.access(3);
        test.access(5);
        test.access(1);
        test.access(1);
        test.access(4);
        test.access(1);
        test.access(2);
        test.access(1);
        test.access(3);
        test.access(1);
        assert_eq!(test.count_valid(), 4);
        assert_eq!(
            test.valid,
            [
                true, true, true, true, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false
            ]
        );
        assert_eq!(
            test.clock.accessed,
            [
                false, true, true, true, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false
            ]
        );
    }
}
