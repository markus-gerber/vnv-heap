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

pub struct SlicedStorageModule<const SLICE_SIZE: usize, S: PersistentStorageModule> {
    inner: S,
}

impl<const SLICE_SIZE: usize, S: PersistentStorageModule> SlicedStorageModule<SLICE_SIZE, S> {
    pub fn new(storage: S) -> Self {
        Self {
            inner: storage
        }
    }
}

impl<const SLICE_SIZE: usize, S: PersistentStorageModule> PersistentStorageModule for SlicedStorageModule<SLICE_SIZE, S> {
    fn read(&mut self, offset: usize, dest: &mut [u8]) -> Result<(), ()> {
        let mut rel_offset = 0;
        while rel_offset < dest.len() {
            let end_read = (rel_offset + SLICE_SIZE).min(dest.len());
            self.inner.read(offset + rel_offset, &mut dest[rel_offset..end_read])?;

            rel_offset += SLICE_SIZE;
        }
        
        Ok(())
    }

    fn get_max_size(&self) -> usize {
        self.inner.get_max_size()
    }

    fn write(&mut self, offset: usize, src: &[u8]) -> Result<(), ()> {
        let mut rel_offset = 0;
        while rel_offset < src.len() {
            let end_read = (rel_offset + SLICE_SIZE).min(src.len());
            self.inner.write(offset + rel_offset, &src[rel_offset..end_read])?;

            rel_offset += SLICE_SIZE;
        }
        Ok(())
    }
}
