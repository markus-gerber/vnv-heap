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

use super::PersistentStorageModule;

pub struct TruncatedStorageModule<const SIZE: usize, S: PersistentStorageModule> {
    inner: S,
}

impl<const SIZE: usize, S: PersistentStorageModule> TruncatedStorageModule<SIZE, S> {
    pub fn new(storage: S) -> Self {
        assert!(storage.get_max_size() >= SIZE);

        Self {
            inner: storage
        }
    }
}

impl<const SIZE: usize, S: PersistentStorageModule> PersistentStorageModule for TruncatedStorageModule<SIZE, S> {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        debug_assert!(offset + dest.len() <= SIZE);
        self.inner.read(offset, dest)
    }

    fn get_max_size(&self) -> usize {
        SIZE
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        debug_assert!(offset + src.len() <= SIZE);
        self.inner.write(offset, src)
    }
}
