#include <version.h>
#if KERNEL_VERSION_MAJOR < 3
#include <zephyr.h>
#else
#include <zephyr/kernel.h>
#endif

#include "../../common/atomics/atomics.h"
#include "../../common/spi_fram_storage/include/mb85rs64v_spi_fram.h"

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

void main(void) {
	rust_main();
}
