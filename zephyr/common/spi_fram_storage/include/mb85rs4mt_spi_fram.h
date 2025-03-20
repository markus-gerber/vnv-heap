/*
 * Copyright (c) 2016 Intel Corporation
 *
 * SPDX-License-Identifier: Apache-2.0
 */
// heavily modified to 

#ifndef SPI_FRAM_H
#define SPI_FRAM_H

#include <errno.h>
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <zephyr/device.h>
#include <zephyr/drivers/spi.h>

#define MB85RS4MT_MANUFACTURER_ID_CMD 0x9f
#define MB85RS4MT_WRITE_ENABLE_CMD 0x06
#define MB85RS4MT_READ_CMD 0x03
#define MB85RS4MT_WRITE_CMD 0x02

struct spi_dt_spec mb85rs4mt_init(int* error) {
	struct spi_config spi_cfg = {
		.frequency = 40000000U,
		.operation = SPI_WORD_SET(8),
		.cs = SPI_CS_CONTROL_INIT(DT_NODELABEL(spidev), 10),
	};

	const struct device* device = DEVICE_DT_GET(DT_NODELABEL(spi2));
	if (!device_is_ready(device)) {
		*error = 1;

		struct spi_dt_spec spec = {
			.bus = NULL,
			.config = spi_cfg
		};

		return spec;
	}

	struct spi_dt_spec spec = {
		.bus = device,
		.config = spi_cfg
	};
	
	return spec;
}

static inline int mb85rs4mt_access(const struct spi_dt_spec* device,
			    uint8_t cmd, uint32_t addr, void *data, size_t len)
{
	uint8_t access[4];
	struct spi_buf bufs[] = {
		{
			.buf = access,
		},
		{
			.buf = data,
			.len = len
		}
	};
	struct spi_buf_set tx = {
		.buffers = bufs
	};

	access[0] = cmd;

	if (cmd == MB85RS4MT_WRITE_CMD || cmd == MB85RS4MT_READ_CMD) {
		access[1] = (addr >> (8 * 2)) & 0xFF;
		access[2] = (addr >> (8 * 1)) & 0xFF;
		access[3] = (addr >> (8 * 0)) & 0xFF;

		bufs[0].len = 4;
		tx.count = 2;

		if (cmd == MB85RS4MT_READ_CMD) {
			struct spi_buf_set rx = {
				.buffers = bufs,
				.count = 2
			};

			return spi_transceive_dt(device, &tx, &rx);
		}
	} else {
		tx.count = 1;
		bufs[0].len = 1;
	}

	return spi_write_dt(device, &tx);
}


int mb85rs4mt_validate_id(const struct spi_dt_spec* device)
{
	uint8_t id[4];

	uint8_t cmd = MB85RS4MT_MANUFACTURER_ID_CMD;
	struct spi_buf bufs[] = {
		{
			.buf = &cmd,
			.len = 1
		},
		{
			.buf = id,
			.len = sizeof(id)
		}
	};
	struct spi_buf_set tx = {
		.buffers = bufs,
		.count = 1
	};
	struct spi_buf_set rx = {
		.buffers = bufs,
		.count = 2
	};

	int err;
	err = spi_transceive_dt(device, &tx, &rx);

	if (err) {
		return -EIO;
	}

	if (id[0] != 0x04) {
		return -EIO;
	}

	if (id[1] != 0x7f) {
		return -EIO;
	}

	if (id[2] != 0x48) { // in spec 0x49 is specified??
		return -EIO;
	}

	if (id[3] != 0x03) {
		return -EIO;
	}

	return 0;
}

int mb85rs4mt_write_bytes(const struct spi_dt_spec* device,
		       uint32_t addr, const uint8_t *data, uint32_t num_bytes)
{
	int err;

	/* disable write protect */
	err = mb85rs4mt_access(device,
			       MB85RS4MT_WRITE_ENABLE_CMD, 0, NULL, 0);
	if (err) {
		return -EIO;
	}

	/* write cmd */
	err = mb85rs4mt_access(device,
			       MB85RS4MT_WRITE_CMD, addr, (uint8_t*) data, num_bytes);
	if (err) {
		return -EIO;
	}

	return 0;
}

int mb85rs4mt_read_bytes(const struct spi_dt_spec* device,
		      uint32_t addr, uint8_t *data, uint32_t num_bytes)
{
	int err;

	/* read cmd */
	err = mb85rs4mt_access(device,
			       MB85RS4MT_READ_CMD, addr, data, num_bytes);
	if (err) {
		return -EIO;
	}

	return 0;
}

#endif