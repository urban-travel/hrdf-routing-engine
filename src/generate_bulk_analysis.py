import json
import os

import geopandas as gpd
import folium
import glob
import sys
from dataclasses import dataclass
from os.path import getmtime
from typing import Any

from pathlib import Path

import numpy as np
from kaleido._utils import fig_tools
from pandas.core.interchange import column

from choropleth_generator import generate_img_from_hectare
from compute_hectare_display_values import load_and_update_hectare

import matplotlib.pyplot as plt

IMG_PREFIX = "img/tpg"

@dataclass
class MeasureStat:
    max_decrease: float
    max_increase: float
    median: float
    average: float
    winning_nb: float
    losing_nb: float
    percentiles: list
    total_surface: int
    hectare_nb: int = -1

    def __str__(self, column_width=-1) -> str:
        return ""

    # todo: add function to create display list
    def extract_as_list(self, column_width) -> list[float | list [float]]:
        return [
            self.hectare_nb,
            self.max_decrease,
            self.max_increase,
            self.median,
            self.average,
            self.winning_nb,
            self.losing_nb,
            self.percentiles,
        ]
    # todo: add function to create array header
    @staticmethod
    def extract_header_list(column_width) -> list[str]:
        return [
            "hectare_nb".center(column_width),
            "max_decrease".center(column_width),
            "max_increase".center(column_width),
            "median".center(column_width),
            "average".center(column_width),
            "winning_nb".center(column_width),
            "losing_nb".center(column_width),
            "percentiles".center(column_width),
        ]

    @property
    def per10(self):
        return self.percentiles[-len(self.percentiles)//10]

    @property
    def per25(self):
        return self.percentiles[-len(self.percentiles)//4]

@dataclass
class FileStats:
    # Base statistics
    base_metrics: MeasureStat
    filtered_base_metrics: MeasureStat
    # Population weighted
    pop_metrics: MeasureStat
    filtered_pop_metrics: MeasureStat
    # Percentages
    percentage_metrics: MeasureStat
    filtered_percentage_metrics: MeasureStat
    # Percentages weighted by population
    percentage_pop_metrics: MeasureStat
    filtered_percentage_pop_metrics: MeasureStat

    # Global information
    glob_inhabitant_nb: int
    glob_hectares_nb: int


    def get_measure_dict(self):
        content_dict = {"base metrics": self.base_metrics, "filtered base metrics": self.filtered_base_metrics,
                        "population weighted metrics": self.pop_metrics, "filtered population weighted metrics": self.filtered_pop_metrics,
                        "percentage metrics": self.percentage_metrics,
                        "filtered percentage metrics": self.filtered_percentage_metrics,
                        "percentage population weighted metrics": self.percentage_pop_metrics,
                        "filtered percentage population weighted metrics": self.filtered_percentage_pop_metrics,}
        global_values = {"total inhabitant number": self.glob_inhabitant_nb, "total hectares number": self.glob_hectares_nb}
        return content_dict, global_values


import datetime


# Function to convert string to datetime
def convert(date_time):
    format = '%b %d %Y %I:%M%p'
    datetime_str = datetime.datetime.strptime(date_time, format)


def format_percentage(fs_obj, measure, att):
    att_unit = "%" if "pop" not in att else ""
    if "glob" not in att:
        return " (" + str(round(fs_obj[0][base].__getattribute__(measure).__getattribute__(att) * 100, 2)) + f"{att_unit})"
    else:
        return ""


def format_percentage_list(fs_obj, i, measure, att):
    att_unit = "%" if "pop" not in att else ""
    if "glob" not in att:
        return " (" + str(round(fs_obj[0][base].__getattribute__(measure).__getattribute__(att)[i] * 100, 2)) + f"{att_unit})"
    else:
        return ""


def create_display_dict(files_stats: dict[str, dict[int, dict[str, FileStats]]], out_files: list[str]):
    days_from = [datetime.datetime.fromisoformat(fn.split("_")[-2]) for fn in out_files]
    days_to = [datetime.datetime.fromisoformat(fn.split("__")[1].split("_")[0]) for fn in out_files]
    filters = [fn.split("__")[0].split("_", 1)[1] for fn in out_files]
    dates_analysed = {fn.split("__")[1].split("_")[0] for fn in out_files}
    resulting_dict = {}

    for base_ix, base in enumerate(["max", "mid", "min"]):
        resulting_dict[base] = {}
        for date in dates_analysed:
            resulting_dict[base][date] = {}
            for filter in filters:
                for file, values_dict in files_stats.items():
                    if date not in file or filter not in file:
                        continue
                    resulting_dict[base][date][filter] = values_dict[base_ix][base]
    return resulting_dict


def display_hist(values: list[int | float], keys: list[str], metric_name: str, save_img: str | None, save_only: bool):
    fig = plt.figure(layout="constrained", figsize=(40, 24))
    layout = "a"
    axs = fig.subplot_mosaic(layout)

    x = 'a'
    axs[x].set_xlabel('Filter', fontsize=45)
    y_axis_label = f"{metric_name}"
    axs[x].set_ylabel(y_axis_label, fontsize=45)

    plot = axs[x].bar(keys, values)
    axs[x].bar_label(plot, rotation=0, padding=0)
    plt.xticks(rotation=45, ha='right')

    if not save_only:
        plt.show()
    if save_img is not None:
        plt.savefig(f'{save_img}.svg', dpi=150)
    plt.close()


def display_detailed_hist(values: dict[str,list[int | float]], ylabel_name: str, metric_names: list[str], save_img: str | None, save_only: bool, normalize: bool):
    fig = plt.figure(layout="constrained", figsize=(40, 24))
    layout = "a"
    axs = fig.subplot_mosaic(layout)

    bar_width = 0.05
    colors = ['xkcd:mauve', 'xkcd:teal', 'xkcd:purple', 'xkcd:forest green', 'xkcd:sky blue', 'xkcd:salmon',
              'xkcd:dark red', 'xkcd:seafoam', 'xkcd:dark orange', 'xkcd:grey blue', 'xkcd:goldenrod', 'xkcd:rust',
              'xkcd:fuchsia', 'xkcd:pale purple', 'xkcd:crimson', 'xkcd:sage', 'xkcd:coral', 'xkcd:grape', 'xkcd:grass']
    x = 'a'
    # axs[x].set_xlabel('Filter', fontsize=45)
    y_axis_label = f"{ylabel_name}"
    axs[x].set_ylabel(y_axis_label, fontsize=45)
    max_val = [min([v[i] for v in values.values()]) for i in range(len(metric_names))]
    min_val = [max([v[i] for v in values.values()]) for i in range(len(metric_names))]

    br: list = [np.arange(len(metric_names))]
    for i in range(len(values.keys()) - 1):
        br.append([p + bar_width for p in br[-1]])
    for (i, c), (k, v) in zip(enumerate(colors),values.items()):
        if normalize:
            displayed_values = [(val - min_val[i])/(max_val[i] - min_val[i]) if (max_val[i] - min_val[i]) != 0 else 0 for i, val in enumerate(v)]
        else:
            displayed_values = v
        label_values = [int(val * 100)/100 for val in v]
        plot = axs[x].bar(br[i], displayed_values, color=c, width=bar_width, label=k.split("/")[-1].split(".")[0].split("_")[-1].split('l')[-1])
        axs[x].bar_label(plot, labels=label_values, rotation=90)
    plt.xticks([r + bar_width for r in range(len(metric_names))], metric_names)
    plt.legend()

    if not save_only:
        plt.show()
    if save_img is not None:
        plt.savefig(f'{save_img}.svg', dpi=150)
    plt.close()


def get_line_color(filter):
    colors = ['xkcd:mauve', 'xkcd:teal', 'xkcd:purple', 'xkcd:forest green', 'xkcd:sky blue', 'xkcd:salmon',
              'xkcd:dark red', 'xkcd:seafoam', 'xkcd:dark orange', 'xkcd:grey blue', 'xkcd:goldenrod', 'xkcd:rust',
              'xkcd:fuchsia', 'xkcd:pale purple', 'xkcd:crimson', 'xkcd:sage', 'xkcd:coral', 'xkcd:grape', 'xkcd:grass',
              'xkcd:cornflower', 'xkcd:rose pink', 'xkcd:sky', 'xkcd:jade', 'xkcd:dull blue', 'xkcd:dark sky blue',
              'xkcd:dark peach', 'xkcd:light lavender', 'xkcd:electric green', 'xkcd:slate grey', 'xkcd:teal green',
              'xkcd:barney purple', 'xkcd:bright orange', 'xkcd:bluegreen', 'xkcd:blush', 'xkcd:lemon', 'xkcd:forest',
              'xkcd:medium purple', 'xkcd:vermillion', 'xkcd:watermelon']
    line_name = filter.split("/")[-1].split(".")[0].split("_")[-1].split('l')[-1]
    total_value = 0
    factor = 128
    for char in line_name:
        total_value = total_value * factor + ord(char)
    return colors[total_value % len(colors)]


def display_superposed_hist(values: dict[str,list[int | float]], fig_title: str, metric_names: list[str], save_img: str | None, nb_to_display: int, save_only: bool, normalize: bool, sort_bars: bool):
    fig = plt.figure(layout="constrained", figsize=(40, 24))
    layout = "a;b;c;d;e;f"
    axs = fig.subplot_mosaic(layout)
    fig.suptitle(fig_title)

    bar_width = 0.5
    colors = ['xkcd:mauve', 'xkcd:teal', 'xkcd:purple', 'xkcd:forest green', 'xkcd:sky blue', 'xkcd:salmon',
              'xkcd:dark red', 'xkcd:seafoam', 'xkcd:dark orange', 'xkcd:grey blue', 'xkcd:goldenrod', 'xkcd:rust',
              'xkcd:fuchsia', 'xkcd:pale purple', 'xkcd:crimson', 'xkcd:sage', 'xkcd:coral', 'xkcd:grape', 'xkcd:grass',
              'xkcd:cornflower', 'xkcd:rose pink', 'xkcd:sky', 'xkcd:jade', 'xkcd:dull blue', 'xkcd:dark sky blue',
              'xkcd:dark peach', 'xkcd:light lavender', 'xkcd:electric green', 'xkcd:slate grey', 'xkcd:teal green',
              'xkcd:barney purple', 'xkcd:bright orange', 'xkcd:darker green', 'xkcd:blush', 'xkcd:lemon', 'xkcd:forest',
              ]
    values = sorted(values.items(), key=lambda v: v[1][0], reverse=True)[:nb_to_display]
    values = [(k, v, get_line_color(k))
              for c, (k, v) in zip(colors, values)]
    for j, x in enumerate(layout.split(';')):
        line_names = []
        # axs[x].set_xlabel('Filter', fontsize=45)
        y_axis_label = f"{metric_names[j]}"
        axs[x].set_ylabel(y_axis_label, fontsize=15)
        max_val = [min([v[1][i] for v in values]) for i in range(len(metric_names))]
        min_val = [max([v[1][i] for v in values]) for i in range(len(metric_names))]
        # values = OrderedDict(values.items())
        values = sorted(values, key=lambda v: v[1][j], reverse=True)
        br = np.arange(len(values))
        for i, (k, v, c) in enumerate(values):
            if normalize:
                displayed_values = [(val - min_val[i])/(max_val[i] - min_val[i]) if (max_val[i] - min_val[i]) != 0 else 0 for i, val in enumerate(v)]
            else:
                displayed_values = v
            label_values = [int(val * 100)/100 for val in v]
            line_name = k.split("/")[-1].split(".")[0].split("_")[-1].split('l')[-1]
            line_names.append(line_name)
            plot = axs[x].bar(br[i], displayed_values[j], color=c, width=bar_width, label=line_name)
            axs[x].bar_label(plot, labels=[label_values[j]], rotation=90)
        axs[x].set_xticks([r for r in range(len(line_names))], line_names)
        # plt.xticks([r + bar_width for r in range(len(line_names))], line_names)
        # plt.legend()

    if not save_only:
        plt.show()
    if save_img is not None:
        plt.savefig(f'{save_img}.svg', dpi=150)
    plt.close()


def generate_header(first_datetime, last_datetime):

    first_time = first_datetime.time()
    last_time = last_datetime.time()
    first_day = first_datetime.date()
    last_day = last_datetime.date()
    format_with_weekday = "%A %Y-%m-%d"
    lines = [
        f"Summary of analysis from the {first_day.strftime(format_with_weekday)} at {first_time} to the {last_day.strftime(format_with_weekday)} at {last_time}"]
    lines += ['Résultat des comparaisons entre les filtres annoncés.',
              "Nous comparons les surfaces de l'isochrone minimal, le médian et le maximal.",
              "Pour chacune de ces mesures, nous comparons à différentes dates et heures.",
              "Pour chacune de ces datres, nous comparons plusieurs métriques :",
              " - La différence de surface disponible depuis chaque hectare en m²",
              " - La différence de surface disponible depuis chaque hectare en % perdu ou gagné.",
              " - Ces deux métriques pondérées par la population de chacun des hectares.",
              " - Ces quatre métriques avec un filtre ignorant les hectares qui ont changé de moins d'1 m²",
              "Pour chacune de ces comparaisons, nous affichons pour chaque filtre les valeurs calculées.",
              "",
              "Description des valeurs :",
              "nb hectares : Le nombre d'hectares qui sont considérés pour cette ligne (ce sera le nombre total pour les cas non filtrés)",
              "Max_decrease : la surface perdue de l'hectare ayant le plus perdu de surface",
              "max_increase : la surface gagnée de l'hectare ayant le plus gagné en surface",
              "median : la médiane de différence de surface",
              "average : la moyenne de différence de surface",
              "winning_nb : le nombre d'hectares ayant gagné en surface",
              "losing_nb : le nombre d'hectares ayant perdu en surface",
              "percentiles : le tableau représentant les percentiles affiché, avec à chaque fois la surface gagnée (ou perdue si négatif) correspondant à ce percentile",
              "les champs suivants, avec pop_, sont équivalents aux champs décris ci-dessus, mais sont pondérés par le nombre d'habitants habitant dans chaque hectare",
              "les champs pop_winning_nb et pop_losing_nb indiquent combien de personnes ont vu la surface de l'isochrone de leur lieu d'habitant se réduire.",
              "inhabitant_nb: Le nombre d'habitants total pour tous les hectares considérés",
              "hectares_nb: Le nombre total d'hectares considérés",
              ]
    lines += [""]
    return lines


def write_summary(files_stats: dict[str, dict[int, dict[str, FileStats]]], out_files: list[str], write_on_disk: bool,
                  print_result: bool, print_by_filter: bool = True):
    lines: list[str] = []
    column_width = 30
    first_column_width = 20
    hist_display_nb = 6
    wanted_hist = ["hectare_nb", "max_increase", "median", "total_surface", "per10", "per25"]
    display_dict: dict[str, dict[str, dict[str, FileStats]]] = create_display_dict(files_stats, out_files)

    days_to = [datetime.datetime.fromisoformat(fn.split("__")[1].split("_")[0]) for fn in out_files]
    first_datetime = min(days_to)
    last_datetime = max(days_to)
    lines += generate_header(first_datetime, last_datetime)
    first = True
    measures = list(list(list(display_dict.values())[0].values())[0].values())[0].get_measure_dict()[0].keys()
    interesting_ones = []

    for base, d in display_dict.items():
        for date, val in d.items():
            for measure_name in measures:
                for filter, v in val.items():
                    local_measures, local_glob = v.get_measure_dict()
                    measure = local_measures[measure_name]
                    if first:
                        lines += [f"{measure_name}:"]
                        lines += [" " * first_column_width + "|" + "|".join(measure.extract_header_list(column_width))]
                        first = False
                    lines += ["|".join([filter.split("/")[-1].split(".")[0].center(first_column_width)] + [str(e).center(column_width) for e in measure.extract_as_list(column_width)])]
                    lines += ["_" * len(lines[-1])]

                lines += ["Comments:"]
                lines += [""]
                lines += [""]
                lines += ["=" * len(lines[-1])]
                lines += [""]
                first = True
                for hist_attr in wanted_hist:
                    values_to_display = [e.get_measure_dict()[0][measure_name].__getattribute__(hist_attr) for e in val.values()]
                    interesting_ones += sorted(list(val.items()), key=lambda tu: tu[1].get_measure_dict()[0][measure_name].__getattribute__(hist_attr))[-5:]
                    keys = [key.split("/")[-1].split(".filter")[0] for key in val.keys()]
                    folder_name = f"{IMG_PREFIX}/{measure_name}_{base}"
                    if not Path(folder_name).exists():
                        os.makedirs(folder_name)
                    fn = f"{folder_name}/hist_{hist_attr}_{date}"
                    if not Path(fn).exists():
                        display_hist(values_to_display, keys, hist_attr, fn, True)

                interesting_ones = {key: [val.get_measure_dict()[0][measure_name].__getattribute__(hist_attr) for hist_attr in wanted_hist]
                                        for (key, val) in interesting_ones}
                interesting_ones_invert = {hist: [v[i] for k, v in interesting_ones.items()] for i, hist in enumerate(wanted_hist)}
                folder_name = f"{IMG_PREFIX}/{measure_name}_{base}"
                fn = f"{folder_name}/detailed_hist_{date}"
                if not Path(fn).exists() or True:
                    display_superposed_hist(interesting_ones, f"Most impactful lines for {folder_name} {date}", wanted_hist, fn, hist_display_nb, True, False, True)
                fn = f"{folder_name}/detailed_hist_inv_{date}"
                if not Path(fn).exists() or True:
                    display_detailed_hist(interesting_ones, "surface modified", wanted_hist, fn, hist_display_nb, True, True)
                interesting_ones = []
    if print_result:
        print("\n".join(lines))
    if write_on_disk:
        with open("summary_" + first_datetime.isoformat() + "_" + last_datetime.isoformat() + ".stat", "w") as f:
            f.write("\n".join(lines))


def old_write_summary(files_stats: dict[str, dict[int, dict[str, FileStats]]], out_files: list[str], write_on_disk: bool,
                  print_result: bool, print_by_filter: bool = True):
    format_with_weekday = "%A %Y-%m-%d"
    column_width = 30

    days_from = [datetime.datetime.fromisoformat(fn.split("_")[-2]) for fn in out_files]
    days_to = [datetime.datetime.fromisoformat(fn.split("__")[1].split("_")[0]) for fn in out_files]
    dates_analysed = {datetime.datetime.fromisoformat(fn.split("__")[1].split("_")[0]) for fn in out_files}
    filters = [fn.split("__")[0].split("_", 2)[1] for fn in out_files]
    bases = list(files_stats.values())[0][0].keys()
    first_datetime = min(days_to)
    last_datetime = max(days_to)
    first_time = first_datetime.time()
    last_time = last_datetime.time()
    first_day = first_datetime.date()
    last_day = last_datetime.date()
    lines: list[str] = []
    lines += [
        f"Summary of analysis from the {first_day.strftime(format_with_weekday)} at {first_time} to the {last_day.strftime(format_with_weekday)} at {last_time}"]
    lines += ['Résultat des comparaisons sur une semaine, entre les horaires 2024 et 2025.',
              "Nous comparons les surfaces de l'isochrone minimal, le médian et le maximal.",
              "Pour chacune de ces comparaisons, nous affichons pour chaque jour les valeurs calculées.",
              "",
              "Description des valeurs :",
              "Max_decrease : la surface perdue de l'hectare ayant le plus perdu de surface",
              "max_increase : la surface gagnée de l'hectare ayant le plus gagné en surface",
              "median : la médiane de différence de surface",
              "average : la moyenne de différence de surface",
              "winning_nb : le nombre d'hectares ayant gagné en surface",
              "losing_nb : le nombre d'hectares ayant perdu en surface",
              "percentiles : le tableau représentant les percentiles affiché, avec à chaque fois la surface gagnée (ou perdue si négatif) correspondant à ce percentile",
              "les champs suivants, avec pop_, sont équivalents aux champs décris ci-dessus, mais sont pondérés par le nombre d'habitants habitant dans chaque hectare",
              "les champs pop_winning_nb et pop_losing_nb indiquent combien de personnes ont vu la surface de l'isochrone de leur lieu d'habitant se réduire.",
              "inhabitant_nb: Le nombre d'habitants total pour tous les hectares considérés",
              "hectares_nb: Le nombre total d'hectares considérés",
              ]
    lines += [""]
    for num, base in zip(["First", "Second", "Third", "Fourth, Fifth"], bases):
        for date in dates_analysed:

            lines += [f"{num} array giving values for {base} comparisons for the {date}:"]
            lines += [""]
            # todo: Change header and array orientation
            lines += [" | ".join(
                ["Statistic".center(column_width)] + [dt.date().strftime(format_with_weekday).center(column_width) for dt in
                                                      days_to])]
            lines += ["_" * len(lines[-1])]
            for measure in files_stats[out_files[0]][0][base].__dict__.keys():
                att_unit = "%" if "pop" not in measure else ""
                if isinstance(measure, MeasureStat):
                    for att in files_stats[out_files[0]][0][base].__getattribute__(measure).__dict__.keys():
                        if type(files_stats[out_files[0]][0][base].__getattribute__(measure).__getattribute__(att)) != list:
                            lines += [" | ".join(
                                [att.center(column_width)] + [(str(round(fs_obj[0][base].__getattribute__(measure).__getattribute__(att), 3)) + format_percentage(fs_obj, measure, att)).center(column_width)
                                                              for fs_obj in files_stats.values()])]
                        else:
                            lists = [fs_obj[0][base].__getattribute__(measure).__getattribute__(att) for fs_obj in files_stats.values()]
                            for i in range(len(lists[0])):
                                lines += [" | ".join([(att if i == 0 else "").center((column_width * 2) // 3)] + [
                                    (str(i * 100 // len(lists[0])) + "%").center(column_width // 3 - 3)] + [
                                                         (str(round(fs_obj[0][base].__getattribute__(measure).__getattribute__(att)[i], 3)) + format_percentage_list(fs_obj, i, measure, att)).center(column_width) for
                                                         fs_obj in files_stats.values()])]
                        lines += ["-" * len(lines[-1])]
            lines += ["Comments:"]
            lines += [""]
            lines += [""]
            lines += ["=" * len(lines[-1])]
            lines += [""]
        lines += ["*" * len(lines[-1])]
        lines += ["*" * len(lines[-1])]

    if print_result:
        print("\n".join(lines))
    if write_on_disk:
        with open("summary_" + first_datetime.isoformat() + "_" + last_datetime.isoformat() + ".stat", "w") as f:
            f.write("\n".join(lines))


def compute_stats(hectares: list[dict], attribute, wanted_percentiles, lines, factor) -> tuple[MeasureStat, MeasureStat, str]:
    hectares.sort(key=lambda v: v[attribute] * factor(v))
    max_decrease = hectares[0][attribute] * factor(hectares[0])
    max_increase = hectares[-1][attribute] * factor(hectares[-1])
    median = hectares[total_hectares // 2][attribute] * factor(hectares[total_hectares // 2])
    avg = sum(v[attribute] * factor(v) for v in hectares) / total_hectares
    winning_nb = sum(1 for v in hectares if v[attribute] > 0)
    losing_nb = sum(1 for v in hectares if v[attribute] < 0)
    total_surface = sum(factor(v) * v[attribute] for v in hectares)
    percentiles = []
    lines += f"Highest loss : {max_decrease}\n"
    lines += f"Highest increase : {max_increase}\n"
    lines += f"Median : {median}\n"
    lines += f"Average: {avg}\n"
    lines += f"Winning hectares: {winning_nb}\n"
    lines += f"Losing hectares: {losing_nb}\n"
    lines += f"For the percentile | we have at most won | m²\n"
    lines += f"_____________________________________________\n"
    for p in range(wanted_percentiles):
        concerned_hectare = hectares[(total_hectares * p) // wanted_percentiles]
        percentile = concerned_hectare[attribute] * factor(concerned_hectare)
        percentiles.append(percentile)
        p_line = f"{p * 100 / wanted_percentiles}% | {percentile} m²\n"
        lines += p_line
    total_stats = MeasureStat(max_decrease, max_increase, median, avg, winning_nb, losing_nb, percentiles, total_surface, total_hectares)
    # todo: evaluate and compute distance of the modification metric
    
    # add stats for only modified hectares
    validity_threshold: int = 1
    tmp_hectares = [h for h in hectares if abs(h[attribute]) > validity_threshold]
    hectares_nb = len(tmp_hectares)
    if hectares_nb > 0:
        max_decrease = tmp_hectares[0][attribute] * factor(tmp_hectares[0])
        max_increase = tmp_hectares[-1][attribute] * factor(tmp_hectares[-1])
        median = tmp_hectares[hectares_nb // 2][attribute] * factor(tmp_hectares[hectares_nb // 2])
        avg = sum(v[attribute] * factor(v) for v in tmp_hectares) / hectares_nb
        winning_nb = sum(1 for v in tmp_hectares if v[attribute] > 0)
        losing_nb = sum(1 for v in tmp_hectares if v[attribute] < 0)
        total_surface = sum(factor(v) * v[attribute] for v in tmp_hectares)
        percentiles = []
        lines += f"#############################################\n"
        lines += f"Statistics only for hectares having a modification of at least {validity_threshold}\n"
        lines += f"Number of impacted hectared : {hectares_nb}\n"
        lines += f"Highest loss : {max_decrease}\n"
        lines += f"Highest increase : {max_increase}\n"
        lines += f"Median : {median}\n"
        lines += f"Average: {avg}\n"
        lines += f"Winning hectares: {winning_nb}\n"
        lines += f"Losing hectares: {losing_nb}\n"
        lines += f"For the percentile | we have at most won | m²\n"
        lines += f"_____________________________________________\n"
        for p in range(wanted_percentiles):
            concerned_hectare = tmp_hectares[(hectares_nb * p) // wanted_percentiles]
            percentile = concerned_hectare[attribute] * factor(concerned_hectare)
            percentiles.append(percentile)
            p_line = f"{p * 100 / wanted_percentiles}% | {percentile} m²\n"
            lines += p_line
    filter_stats = MeasureStat(max_decrease, max_increase, median, avg, winning_nb, losing_nb, percentiles, total_surface, hectares_nb)
    return total_stats, filter_stats, lines


if __name__ == "__main__":
    generate_img = 0
    generate_stats = True
    print_stat = False
    compute_summary = True
    wanted_percentiles = 20
    hectare_files = []
    if len(sys.argv) >= 2:
        hectare_file = sys.argv[1]
        hectare_files.append(hectare_file)
    else:
        hectare_files = glob.glob("hectare_resources/hectare_*.json")

    geojson_filename = "geo_ressources/geo_grid_swiss_wgs84_combined_filtered"
    with open(geojson_filename + '.geojson') as geo_file:
        # with open('geo_ressources/geo_romandie_wgs84_PROD_v1.json') as geo_file:
        region_map = json.load(geo_file)
        geodata = gpd.read_file(geojson_filename + '.geojson')
        geodata.plot()
        # plt.show()
        plt.close()

    base_files = [fil for fil in hectare_files if "base" in fil]
    base_hectares = {}
    for base_file in base_files:
        base_hectares[base_file] = load_and_update_hectare(base_file, False)
    hectare_files = [fil for fil in hectare_files if "base" not in fil]
    summary = {}
    for hectare_file in hectare_files:
        data_modified_time = getmtime(hectare_file)

        try:
            file_time = hectare_file.split(".filter_")[1].split("_PT")[0]
            base_hectare = base_hectares[base_files[0]] if len(base_files) > 0 else False
            for bf, bh in base_hectares.items():
                if file_time in bf:
                    base_hectare = bh
            hectares, nb = load_and_update_hectare(hectare_file, base_hectare)
        except Exception as e:
            print(f"File {hectare_file} has bad format : {e}", file=sys.stderr)
            continue

        summary[hectare_file] = {}
        available_attributes = [key for key in hectares[0].keys() if "-" in key]

        for i, attribute in enumerate(available_attributes):
            # for i in range(nb):
                summary[hectare_file][i] = {}
                for base_ix, base in enumerate(["max", "mid", "min"]):
                    # attribute = attribute.split("_")[0] + "_diff_" + base + "_" + str(i)
                    stat_filename = hectare_file.split(".json")[0] + "_" + attribute + ".stat"
                    img_filename = f"{IMG_PREFIX}/{hectare_file.split("/")[-1].split(".json")[0]}_{attribute}.svg"
                    if (generate_stats and (not Path(stat_filename).exists() or getmtime(stat_filename) < data_modified_time)) or print_stat or compute_summary:
                        total_hectares = len(hectares)
                        total_inhabitant = sum(v["population"] for v in hectares)

                        # get all base stats
                        total_stat, filtered_stats, lines = compute_stats(hectares, attribute, wanted_percentiles, "", lambda x: 1)

                        # Now computing the same values but weighted by concerned population
                        pop_total_stat, pop_filtered_stats, pop_lines = compute_stats(
                            hectares, attribute, wanted_percentiles, "", lambda x: x["population"])

                        # Now computing the same values but in percentage
                        per_total_stat, per_filtered_stats, per_lines = compute_stats(
                            hectares, attribute, wanted_percentiles, "", lambda x: 100.0/x[f"ref_{base}"])

                        # Now computing the same values but weighted by concerned population
                        perpop_total_stat, perpop_filtered_stats, perpop_lines = compute_stats(
                            hectares, attribute, wanted_percentiles, "", lambda x: 100.0*x["population"]/(total_inhabitant*x[f"ref_{base}"]))

                        if print_stat:
                            print(lines)

                        # todo: add global analysis with comparison between lines
                        summary[hectare_file][i][base] = FileStats(base_metrics=total_stat,
                                                                   filtered_base_metrics=filtered_stats,
                                                                   pop_metrics=pop_total_stat,
                                                                   filtered_pop_metrics=pop_filtered_stats,
                                                                   percentage_metrics=per_total_stat,
                                                                   filtered_percentage_metrics=per_filtered_stats,
                                                                   percentage_pop_metrics=perpop_total_stat,
                                                                   filtered_percentage_pop_metrics=perpop_filtered_stats,
                                                                   glob_inhabitant_nb=total_inhabitant,
                                                                   glob_hectares_nb=total_hectares,
                                                                   )

                        if generate_stats and (not Path(stat_filename).exists() or getmtime(stat_filename) < data_modified_time):
                            with open(
                                    stat_filename,
                                    "w") as f:
                                f.write(lines)

                    if generate_img:
                        m = geodata.explore(
                            column="reli",
                            scheme="naturalbreaks",  # use mapclassify's natural breaks scheme
                            legend=True,  # show legend
                        )
                        folium.TileLayer("CartoDB positron", show=False).add_to(
                            m
                        )
                        m
                        if not Path(img_filename).exists() or getmtime(img_filename) < data_modified_time:
                            generate_img_from_hectare(hectares, region_map, attribute,
                                                      img_filename)
                        if generate_img > 1:
                            # attribute = "area_" + str(i + 1) + "_" + base
                            add_img_filename = f"{IMG_PREFIX}/{hectare_file.split("/")[-1].split(".json")[0]}_{attribute}.svg"
                            if not Path(add_img_filename).exists() or getmtime(add_img_filename) < data_modified_time:
                                generate_img_from_hectare(hectares, region_map, attribute, add_img_filename)

    if compute_summary:
        print("Start writing summary")
        write_summary(summary, hectare_files, generate_stats, print_stat)
