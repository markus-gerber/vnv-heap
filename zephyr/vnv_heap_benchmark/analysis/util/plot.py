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

    if "VNV_HEAP_PAPER_DIR" in os.environ:
        paper_dir = os.environ["VNV_HEAP_PAPER_DIR"]
        plt.savefig(f"{paper_dir}/assets/{name}.pdf", bbox_inches='tight')

    if save_asset:
        plt.savefig(f"../../../assets/{name}_plot.svg", bbox_inches='tight')

def set_theme(colors=2, skip=0, ignore=-1, hide_spines=False):
    style = sns.axes_style("whitegrid")
    style["grid.color"] = "#ddd"
    style["axes.edgecolor"] = "#ddd"
    if hide_spines:
        style["axes.spines.right"] = True
        style["axes.spines.top"] = True

    sns.set_theme(style=style)
    sns.set_context("paper", rc={"font.size":8, "font.family": "Libertine", "axes.titlesize":8, "axes.labelsize":8, "xtick.labelsize": 8, "ytick.labelsize": 8, "legend.title_fontsize": 9})

    #palette = sns.color_palette("mako", n_colors=colors)[skip:]
    #if ignore != -1:
    #    palette.pop(ignore)
    if colors == 1:
        palette = [plot_colors["heap"]]
    elif colors == 2:
        palette = [plot_colors["baseline"], plot_colors["heap"]]
    elif colors == 3:
        palette = [plot_colors["baseline"], plot_colors["baseline2"], plot_colors["heap"]]

    sns.set_palette(palette=palette)
    return palette


# class LinePlotEntry(NamedTuple):
#     name: str
#     x: str
#     y: str
#     marker: str
#     use_edge_color: bool | None
#     data: pd.DataFrame
    
# class PlotLinesOptions(NamedTuple):
#     x_label: str | None
#     y_label: str | None
#     scale: Literal['ms', 'us', 'µs', 'ns']
#     data: list[LinePlotEntry]
#     legend_cols: int | None
#     norm: float | None
#     height: float | None
#     width: float | None
#     title: str | None

def plot_lines(options: dict | list[dict]):
    if type(options) == dict:
        option_list = [options]
    elif type(options) == list:
        option_list = options
    else:
        raise Exception("illegal input")

    figheight = 3.3
    if "height" in option_list[0]:
        figheight *= option_list[0]["height"]

    figwidth = 3.3
    if "width" in option_list[0]:
        figwidth *= option_list[0]["width"]

    palette = set_theme(colors=len(option_list[0]["data"]))

    (fig, axes) = plt.subplots(1, len(option_list), figsize=(figwidth, figheight), sharey=True)
    if len(option_list) == 1:
        axes = [axes]
        
    for (i, options) in enumerate(option_list):
        ax = axes[i]

        ax.set_xmargin(0)
        ax.set_ymargin(0)

        x_label = options["x_label"]
        y_label = options["y_label"]
        scale = options["scale"]
        data = options["data"]

        min_x = 0
        max_x = 0
        for (line_data, i) in zip(data, range(0, len(data))):
            data_curr = scale_data(line_data["data"], scale)

            if "norm" in options:
                norm = options["norm"]

                data_curr["mean"] /= norm
                data_curr["min"] /= norm
                data_curr["max"] /= norm
            
            if "use_edge_color" in line_data and line_data["use_edge_color"]:
                markerfacecolor = "None"
                markeredgecolor = palette[i]
            else:
                markerfacecolor = palette[i]
                markeredgecolor = "None"
            
            sns.lineplot(
                ax=ax,
                x=data_curr[line_data["x"]],
                y=data_curr[line_data["y"]],
                label=line_data["name"],
                markers=["o"],
                marker=line_data["marker"],
                markerfacecolor=markerfacecolor,
                markeredgecolor=markeredgecolor,
                color="#bbb",
                linewidth=1,
                dashes=True,
                clip_on=False,
                zorder=1,
            )

            sns.scatterplot(
                ax=ax,
                x=data_curr[line_data["x"]],
                y=data_curr[line_data["y"]],
                markers=["o"],
                marker=line_data["marker"],
                facecolor=markerfacecolor,
                edgecolor=markeredgecolor,
                linewidth=1,
                clip_on=False,
                zorder=3,
            )
            
            for line in ax.lines:
                line.set_linestyle("--")

            min_x = min(min_x, min(data_curr[line_data["x"]]))
            max_x = max(max_x, max(data_curr[line_data["x"]]))

        # set_grid(64, max_x, ax)

        if x_label:
            ax.set_xlabel(x_label)

        if y_label:
            ax.set_ylabel(y_label)
            
        if "title" in options:
            ax.set_title(options["title"], fontweight='bold')

        ncol = 0
        if "legend_cols" in options:
            ncol = options["legend_cols"]
        else:
            ncol = len(ax.legend().get_lines())

        if len(ax.get_legend().legend_handles) != 0:
            if "title" in options:
                ax.legend(
                    loc = "lower center",
                    bbox_to_anchor=(.5, 1.1),
                    ncol=len(ax.get_legend().legend_handles),
                    title=None,
                    frameon=False
                )
            else:
                ax.legend(
                    loc = "lower center",
                    bbox_to_anchor=(.5, 1),
                    ncol=len(ax.get_legend().legend_handles),
                    title=None,
                    frameon=False
                )

    return axes

plot_colors = {
    "heap": sns.color_palette("tab10")[0], # blue
    "baseline": sns.color_palette("tab10")[3], # red
    "baseline2": sns.color_palette("tab10")[1] # orange
}

