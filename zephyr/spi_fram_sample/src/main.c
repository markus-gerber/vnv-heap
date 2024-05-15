#include <version.h>
#if KERNEL_VERSION_MAJOR < 3
#include <zephyr.h>
#else
#include <zephyr/kernel.h>
#endif

#include "atomics.h"
#include "mb85rs64v_spi_fram.h"


extern void rust_main(void);

void main(void)
{
	rust_main();
}
