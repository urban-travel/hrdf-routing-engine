import plotly.express as px
import json

def generate_img_from_hectare(data_to_display: list[dict[str,float]], region_to_display, displayed_value:str, output_filename:str):
    wanted_range = [0,max([v[displayed_value] for v in data_to_display])] if "area" in displayed_value else None
    fig = px.choropleth(data_to_display,
                        geojson=region_to_display,
                        locations="reli",
                        featureidkey="properties.reli",
                        range_color=wanted_range,
                        color_continuous_midpoint=0,
                        color=displayed_value,
                        color_continuous_scale='RdYlGn',
                        projection="mercator")
    # fig.update_geos(fitbounds="locations", visible=False)
    fig.update_geos(
        # showcoastlines=True, coastlinecolor="RebeccaPurple",
        # showland=True, landcolor="LightGreen",
        # showocean=True, oceancolor="LightBlue",
        showlakes=False, lakecolor="Blue",
        showrivers=True, rivercolor="Blue",
        showcountries=True, countrycolor="RebeccaPurple",
        showsubunits=True, subunitcolor="Black",
        resolution=50,
        fitbounds="locations",# visible=False
    )
    # fig.update_geos(fitbounds="locations")
    fig.write_image(file=output_filename, scale=100, width=8000, height=8000)
    fig = px.line_geo(lat=[0,15,20,35], lon=[5,10,25,30])

if __name__ == "__main__":
    hectare_filename = "hectare_all_2026-04-17 07:30:00_2025-04-18 07:30:00_PT1800S"
    geojson_filename = "geo_ressources/geo_grid_swiss_wgs84_combined_filtered"
    with open(geojson_filename + '.geojson') as geo_file:
        # with open('geo_ressources/geo_romandie_wgs84_PROD_v1.json') as geo_file:
        region_map = json.load(geo_file)


    with open(hectare_filename + '.json') as file:
        hectares = json.load(file)
    generate_img_from_hectare(hectares, region_map, "area_max_0.svg", hectare_filename)

