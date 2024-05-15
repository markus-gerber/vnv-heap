// pub(crate) mod debug;

use core::alloc::{Layout, LayoutError};

pub(crate) mod multi_linked_list;

pub(crate) fn padding_needed_for(offset: usize, alignment: usize) -> usize {
    let misalignment = offset % alignment;
    if misalignment > 0 {
        // round up to next multiple of `alignment`
        alignment - misalignment
    } else {
        // already a multiple of `alignment`
        0
    }
}

#[inline]
pub(crate) fn repr_c_layout(fields: &[Layout]) -> Result<Layout, LayoutError> {
    let mut layout = Layout::from_size_align(0, 1)?;
    for &field in fields {
        let (new_layout, _) = layout.extend(field)?;
        layout = new_layout;
    }
    // Remember to finalize with `pad_to_align`!
    Ok(layout.pad_to_align())
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