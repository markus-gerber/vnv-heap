// pub(crate) mod debug;

use core::alloc::{Layout, LayoutError};

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

#[inline]
pub(crate) const fn round_up_to_nearest(num: usize, multiple: usize) -> usize {
    ((num + multiple - 1) / multiple) * multiple
}

#[inline]
pub(crate) const fn div_ceil(num: usize, div: usize) -> usize {
    (num + div - 1) / div
}
