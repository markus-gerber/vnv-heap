# some utils that help with plotting the measurements

import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
import os
import pandas as pd
from util.convert import scale_data
from typing import NamedTuple, Literal

def set_grid(grid_offset: int, max_x: int, ax):
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
    return palette


class LinePlotEntry(NamedTuple):
    name: str
    x: str
    y: str
    marker: str
    data: pd.DataFrame
    
class PlotLinesOptions(NamedTuple):
    x_label: str | None
    y_label: str | None
    scale: Literal['ms', 'us', 'µs', 'ns']
    data: list[LinePlotEntry]
    legend_cols: int | None
    

def plot_lines(options: PlotLinesOptions):
    x_label = options["x_label"]
    y_label = options["y_label"]
    scale = options["scale"]
    data = options["data"]

    palette = set_theme(colors=len(data))

    fig = plt.figure(1)
    fig.set_figheight(6)
    fig.set_figwidth(10)

    ax = plt.subplot()

    min_x = 0
    max_x = 0
    for (line_data, i) in zip(data, range(0, len(data))):
        data_scaled = scale_data(line_data["data"], scale)
        sns.lineplot(
            ax=ax,
            x=data_scaled[line_data["x"]],
            y=data_scaled[line_data["y"]],
            label=line_data["name"],
            markers=["o"],
            marker=line_data["marker"],
            markerfacecolor=palette[i],
            color="#aaaaaa",
            dashes=True
        )
        
        for line in ax.lines:
            line.set_linestyle("--")

        min_x = min(min_x, min(data_scaled[line_data["x"]]))
        max_x = max(max_x, max(data_scaled[line_data["x"]]))

    # set_grid(64, max_x, ax)

    if x_label:
        ax.set_xlabel(x_label)

    if y_label:
        ax.set_ylabel(y_label)

    ncol = 0
    if options["legend_cols"]:
        ncol = options["legend_cols"]
    else:
        ncol = len(ax.legend().get_lines())

    ax.legend(
        loc = "lower center",
        bbox_to_anchor=(.5, 1),
        ncol=ncol,
        title=None,
        frameon=False
    )

    return ax
