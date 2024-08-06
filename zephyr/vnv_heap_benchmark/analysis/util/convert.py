# utils for converting measurement data into the right format
import pandas as pd
import numpy as np
from typing import Literal

def convert_data(raw_data, bench_name: str, columns: list[str]):
    columns = columns.copy()
    columns.append("ticks_per_ms")

    # filter and convert data
    data = filter(lambda item: item["bench_name"] == bench_name, raw_data)

    def convert_item(item):
        new = dict()
        new["mean"] = sum(item["data"]) / len(item["data"])
        new["min"] = min(item["data"])
        new["max"] = max(item["data"])

        for key in item["bench_options"]:
            new["options." + key] = item["bench_options"][key]

        for key in item:
            if key != "bench_options" and key != "data":
                new[key] = item[key]

        return new

    data = pd.DataFrame(list(map(convert_item, data)), columns=columns)

    # make sure machine name, cold start and repetitions match
    if len(np.unique(data["cold_start"])) > 1 or len(np.unique(data["repetitions"])) > 1 or len(np.unique(data["machine_name"])) > 1:
        raise "values should be the same"

    return data

def scale_data(data, scale: Literal["ms", "us", "µs", "ns"]):
    scale_value = {
        "ms": 1,
        "us": 1_000,
        "µs": 1_000,
        "ns": 1_000_000
    }[scale]

    data = data.copy()
    data["mean"] = (scale_value * data["mean"]) / (data["ticks_per_ms"])
    data["min"] = (scale_value * data["min"]) / (data["ticks_per_ms"])
    data["max"] = (scale_value * data["max"]) / (data["ticks_per_ms"])

    return data

def get_storage_measurement(raw_data, max_obj_size: int = -1):
    # filter and convert data
    storage_read = convert_data(raw_data, "persistent_storage_read", ["mean", "min", "max", "options.object_size", "machine_name", "cold_start", "repetitions"])
    storage_write = convert_data(raw_data, "persistent_storage_write", ["mean", "min", "max", "options.object_size", "machine_name", "cold_start", "repetitions"])

    if max_obj_size != -1:
        storage_read = storage_read.loc[storage_read["options.object_size"] <= max_obj_size]
        storage_write = storage_write.loc[storage_write["options.object_size"] <= max_obj_size]

    return (storage_read, storage_write)
