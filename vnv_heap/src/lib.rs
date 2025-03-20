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

mod allocation_identifier;
mod resident_object_manager;
mod persist_access_point;
mod shared_persist_lock;
mod vnv_config;
mod vnv_heap;
mod vnv_list;
mod vnv_list_mut_ref;
mod vnv_list_ref;
mod vnv_array;
mod vnv_array_mut_ref;
mod vnv_mut_ref;
mod vnv_object;
mod vnv_ref;
mod util;

#[cfg(test)]
mod test;

#[cfg(any(feature = "benchmarks", test))]
pub mod benchmarks;

pub use crate::vnv_heap::*;
pub use crate::vnv_object::VNVObject;
pub use crate::vnv_array::VNVArray;
pub use vnv_config::VNVConfig;
pub use vnv_ref::VNVRef;
pub use vnv_mut_ref::VNVMutRef;
pub mod modules;
