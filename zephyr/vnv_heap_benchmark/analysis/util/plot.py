# some utils that help with plotting the measurements

import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
import os

def set_gird(grid_offset: int, max_x: int, ax):
    if max_x % grid_offset != 0:
        max_x = (max_x / grid_offset) * grid_offset
        max_x += grid_offset

    major_ticks = np.arange(0, max_x + 1, grid_offset)
    minor_ticks = np.arange(0, max_x + 1, grid_offset / 2)

    ax.set_xticks(major_ticks)
    ax.set_xticks(minor_ticks, minor=True)

def save_plot(name: str, save_asset: bool = False):
    plt.savefig(f"../figures/{name}.pdf", bbox_inches='tight')

    if "VNV_HEAP_THESIS_DIR" in os.environ:
        thesis_dir = os.environ["VNV_HEAP_THESIS_DIR"]
        plt.savefig(f"{thesis_dir}/figures/plot_{name}.pdf", bbox_inches='tight')

    if save_asset:
        plt.savefig(f"../../../assets/{name}_plot.svg", bbox_inches='tight')

def set_theme(colors=3, skip=0, ignore=-1):
    sns.set_theme()

    palette = sns.color_palette("mako", n_colors=colors)[skip:]
    if ignore != -1:
        palette.pop(ignore)

    sns.set_palette(palette=palette)

