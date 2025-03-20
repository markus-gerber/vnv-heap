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

// This module implements a system inspired by https://dl.acm.org/doi/10.1145/3316781.3317812 (ManagedState)
// This implementation manages multiple pages and limits the number of dirty pages.
// Differences to the previously mentioned paper are:
// 1. Unloading pages is not implemented
// 2. The interface is adapted to vNV-Heap's for better abstraction

mod memory_manager;
mod object;

pub(crate) use memory_manager::{MemoryManager, multi_page_calc_base_metadata_size};

use super::*;
