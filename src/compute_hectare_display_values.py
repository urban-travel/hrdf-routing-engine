import json
import pandas as pd

def load_and_update_hectare(hectare_file: str, base_dict: dict|bool) -> tuple[list, int]:
    file_time = hectare_file.split(".filter_")[1].split("_PT")[0]
    standalone_file = isinstance(base_dict, bool)

    with open(hectare_file) as file:
        hectares = json.load(file)
        pd_hectares = pd.read_json(hectare_file)

    highest_cmp = 0
    filter_base_name = "base"
    for hectare_index, h in enumerate(hectares):
        filters = {}
        # analyze at first each filter individually in case of multiple times
        for key, value in h["area"]:
            previous_max, previous_mid, previous_min = value[0]
            filters[key] = [value[0]]
            if len(value) > highest_cmp:
                highest_cmp = len(value)
            if len(value) < 2 :
                continue
            for i, a in enumerate(value[1:]):
                max, mid, min = a
                if i == 0:
                    h[key + "_area_"+str(i)+"_max"] = max
                    h[key + "_area_"+str(i)+"_mid"] = mid
                    h[key + "_area_"+str(i)+"_min"] = min
                h[key + "_area_"+str(i+1)+"_max"] = max
                h[key + "_area_"+str(i+1)+"_mid"] = mid
                h[key + "_area_"+str(i+1)+"_min"] = min
                h[key + "_diff_max_"+str(i)] = previous_max - max
                h[key + "_diff_mid_"+str(i)] = previous_mid - mid
                h[key + "_diff_min_"+str(i)] = previous_min - min
                previous_max, previous_mid, previous_min = a
                filters[key].append(a)
        # Then compare filters to each other
        for key, value in filters.items():
            if standalone_file:
                if key == filter_base_name:
                    continue
                else:
                    for i, (filtered, base) in enumerate(zip(value, filters[filter_base_name])):
                        for (filtered_val, base_val, metric) in zip(filtered, base, ["max", "mid", "min"]):
                            h[filter_base_name + "-" + key + "_diff_" + metric + "_" + str(i)] = base_val - filtered_val
                            h[f'ref_{metric}'] = base_val
            else:
                for i, (filtered, base) in enumerate(zip(value, base_dict[0][hectare_index]['area'][0][1])):
                    for (filtered_val, base_val, metric) in zip(filtered, base, ["max", "mid", "min"]):
                        h[filter_base_name + "-" + key + "_diff_" + metric + "_" + str(i)] = base_val - filtered_val
                        h[f'ref_{metric}'] = base_val

    return hectares, highest_cmp - 1

def write_hectare(hectare_file: str, hectares: list):
    with open(hectare_file + "_split" + ".json", "w") as out_file:
        json.dump(hectares, out_file)

if __name__ == "__main__":
    hectare_file = "hectare_2025-04-10 07:30:00_PT1800S"
    write_hectare(hectare_file, load_and_update_hectare(hectare_file + '.json')[0])