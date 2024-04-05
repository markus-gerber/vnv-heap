// pub(crate) mod debug;

use libc::{sysconf, _SC_PAGE_SIZE};

pub(crate) fn get_page_size() -> usize {
    unsafe { sysconf(_SC_PAGE_SIZE) as usize }
}

/// efficient way to calculate: ceil(x / y)
pub(crate) fn ceil_div(x: usize, y: usize) -> usize {
    (x + y - 1) / y
}

#[cfg(test)]
mod test {
    use crate::util::ceil_div;

    #[test]
    fn test_ceil_div() {
        // just test a bunch of different values
        for y in 1..100 {
            for x in 0..y*3 {
                let expected_value = if x % y == 0 {
                    x / y
                } else {
                    (x / y) + 1
                };

                assert_eq!(ceil_div(x, y), expected_value);
            }
        }
    }
}