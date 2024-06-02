# some utils that help with plotting the measurements

import numpy as np

def set_gird(grid_offset: int, max_x: int, ax):
    if max_x % grid_offset != 0:
        max_x = (max_x / grid_offset) * grid_offset
        max_x += grid_offset

    major_ticks = np.arange(0, max_x + 1, grid_offset)
    minor_ticks = np.arange(0, max_x + 1, grid_offset / 2)

    ax.set_xticks(major_ticks)
    ax.set_xticks(minor_ticks, minor=True)
