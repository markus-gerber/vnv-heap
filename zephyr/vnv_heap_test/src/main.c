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

void main(void)
{
	int64_t time_stamp;
	int64_t milliseconds_spent;

	time_stamp = k_uptime_get();
	printf("booting at %llims\n", time_stamp);

	rust_main();

	milliseconds_spent = k_uptime_delta(&time_stamp);
	printf("took %llims\n", milliseconds_spent);
}
