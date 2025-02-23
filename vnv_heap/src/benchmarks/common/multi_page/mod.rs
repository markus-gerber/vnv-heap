// This module implements a system inspired by https://dl.acm.org/doi/10.1145/3316781.3317812
// This implementation manages multiple pages and limits the number of dirty pages.
// Differences to the previously mentioned paper are:
// 1. Unloading pages is not implemented
// 2. The interface is adapted to vNV-Heap's for better abstraction

mod memory_manager;
mod object;

pub(crate) use memory_manager::{MemoryManager, multi_page_calc_metadata_size};

use super::*;
