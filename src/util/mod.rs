// pub(crate) mod debug;

use libc::{sysconf, _SC_PAGE_SIZE};

pub(crate) fn get_page_size() -> usize {
    unsafe { sysconf(_SC_PAGE_SIZE) as usize }
}