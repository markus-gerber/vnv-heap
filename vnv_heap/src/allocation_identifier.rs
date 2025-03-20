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

use core::marker::PhantomData;
use std::usize;

pub(crate) struct AllocationIdentifier<T: Sized> {
    pub(crate) offset: usize,
    _phantom_data: PhantomData<T>
}

impl<T: Sized> AllocationIdentifier<T> {
    pub(crate) fn from_offset(offset: usize) -> Self {
        Self {
            offset,
            _phantom_data: PhantomData
        }
    }

    pub(crate) fn new_invalid() -> Self {
        Self {
            _phantom_data: PhantomData,
            offset: usize::MAX
        }
    }

    pub(crate) fn is_invalid(&self) -> bool {
        self.offset == usize::MAX
    }
}

impl<T: Sized> Clone for AllocationIdentifier<T> {
    fn clone(&self) -> Self {
        Self { offset: self.offset.clone(), _phantom_data: self._phantom_data.clone() }
    }
}
