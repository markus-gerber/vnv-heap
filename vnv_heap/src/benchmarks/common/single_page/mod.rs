// This module implements a system containing a single page frame and enables swapping of different pages

mod memory_manager;
mod object;

pub(crate) use memory_manager::MemoryManager;
pub(crate) use object::*;

use super::*;
