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
