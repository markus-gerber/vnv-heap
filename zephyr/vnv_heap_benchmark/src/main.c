/*
 * Copyright (C) 2025  Markus Elias Gerber
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

#include <version.h>
#if KERNEL_VERSION_MAJOR < 3
#include <zephyr.h>
#else
#include <zephyr/kernel.h>
#endif

#include "../../common/atomics/atomics.h"
#include "../../common/spi_fram_storage/include/mb85rs4mt_spi_fram.h"

#include <stdio.h>

extern void rust_main(void);

uint32_t helper_k_cycle_get_32() {
	return k_cycle_get_32();
}

uint32_t helper_sys_clock_hw_cycles_per_sec() {
	return sys_clock_hw_cycles_per_sec();
}

int64_t helper_k_uptime_get() {
	return k_uptime_get();
}

uint64_t helper_irq_lock() {
	int key = irq_lock();
	return key;
}

void helper_irq_unlock(uint64_t key) {
	irq_unlock(key);
}

void main(void) {
	rust_main();
}
