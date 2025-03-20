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

#include <stdio.h>
#include <stdatomic.h>

#include "../../common/atomics/atomics.h"
#include "../../common/spi_fram_storage/include/mb85rs4mt_spi_fram.h"

extern void rust_main(void);

extern void persist(void);

atomic_int last_pressed;

static void button_pressed(const struct device *port, struct gpio_callback *cb, gpio_port_pins_t pins) {
	// debounce button press
	uint32_t curr_time = (uint32_t) k_uptime_get();
	if (curr_time - last_pressed < 500) {
		// last_pressed = curr_time;
		return;
	}
	last_pressed = curr_time;

	// TODO: is printing safe to communicate with UART? If not: make it safe
	printf("persist\n");
	persist();
}

void main(void)
{
	const struct device *port = DEVICE_DT_GET(DT_NODELABEL(gpio0));
	printf("%d\n", gpio_pin_configure(port, 0, GPIO_INPUT | GPIO_PULL_UP));
	printf("%d\n", gpio_pin_interrupt_configure(port, 0, GPIO_INT_EDGE_FALLING));

	struct gpio_callback callback;

    // initialize callback structure for button interrupt
    gpio_init_callback(&callback, button_pressed, BIT(0));

    // attach callback function to button interrupt
    gpio_add_callback(port, &callback);

	rust_main();
}
