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
