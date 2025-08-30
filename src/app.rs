use std::error::Error;

use crate::IsochroneArgs;
use crate::isochrone::{self, IsochroneDisplayMode, compute_isochrones};
use chrono::Duration;
use geo::MultiPolygon;
use hrdf_parser::{Coordinates, Hrdf};
use isochrone::compute_optimal_isochrones;

use self::isochrone::compute_average_isochrones;
use self::isochrone::compute_worst_isochrones;
use self::isochrone::utils::wgs84_to_lv95;

#[allow(clippy::too_many_arguments)]
pub fn run_simple(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args: IsochroneArgs,
    display_mode: IsochroneDisplayMode,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args.time_limit.num_minutes();
    let isochrone_interval = isochrone_args.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args.latitude, isochrone_args.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    #[cfg(feature = "svg")]
    let iso = compute_isochrones(&hrdf, &excluded_polygons, isochrone_args, display_mode);

    #[cfg(feature = "svg")]
    iso.write_svg(
        &format!("isochrones_{}_{}.svg", time_limit, isochrone_interval),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_average(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args.time_limit.num_minutes();
    let isochrone_interval = isochrone_args.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args.latitude, isochrone_args.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    #[cfg(feature = "svg")]
    let iso = compute_average_isochrones(&hrdf, &excluded_polygons, isochrone_args, delta_time);

    #[cfg(feature = "svg")]
    iso.write_svg(
        &format!(
            "average_isochrones_{}_{}_{}.svg",
            time_limit,
            isochrone_interval,
            delta_time.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_optimal(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args.time_limit.num_minutes();
    let isochrone_interval = isochrone_args.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args.latitude, isochrone_args.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    let opt_iso = compute_optimal_isochrones(
        &hrdf,
        &excluded_polygons,
        isochrone_args,
        delta_time,
        display_mode,
    );

    #[cfg(feature = "svg")]
    opt_iso.write_svg(
        &format!(
            "optimal_isochrones_{}_{}.svg",
            time_limit, isochrone_interval
        ),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

pub fn run_worst(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args.time_limit.num_minutes();
    let isochrone_interval = isochrone_args.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args.latitude, isochrone_args.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    let opt_iso = compute_worst_isochrones(
        &hrdf,
        &excluded_polygons,
        isochrone_args,
        delta_time,
        display_mode,
    );

    #[cfg(feature = "svg")]
    opt_iso.write_svg(
        &format!("worst_isochrones_{}_{}.svg", time_limit, isochrone_interval),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_comparison(
    hrdf_2024: Hrdf,
    hrdf_2025: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args_2024: IsochroneArgs,
    isochrone_args_2025: IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args_2024.time_limit.num_minutes();
    let isochrone_interval = isochrone_args_2024.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args_2024.latitude, isochrone_args_2024.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    let isochrones_2024 = compute_optimal_isochrones(
        &hrdf_2024,
        &excluded_polygons,
        isochrone_args_2024,
        delta_time,
        display_mode,
    );
    #[cfg(feature = "svg")]
    isochrones_2024.write_svg(
        &format!("isochrones_2024_{}_{}.svg", time_limit, isochrone_interval),
        1.0 / 100.0,
        Some(coord),
    )?;
    println!(
        "time = {}, surface = {}, max_distance = {}",
        isochrones_2024.departure_at(),
        isochrones_2024.compute_max_area(),
        isochrones_2024.compute_max_distance(coord).1
    );

    let isochrones_2025 = compute_optimal_isochrones(
        &hrdf_2025,
        &excluded_polygons,
        isochrone_args_2025,
        delta_time,
        display_mode,
    );
    #[cfg(feature = "svg")]
    isochrones_2025.write_svg(
        &format!("isochrones_2025_{}_{}.svg", time_limit, isochrone_interval),
        1.0 / 100.0,
        Some(coord),
    )?;
    println!(
        "time = {}, surface = {}, max_distance = {}",
        isochrones_2025.departure_at(),
        isochrones_2025.compute_max_area(),
        isochrones_2025.compute_max_distance(coord).1
    );

    Ok(())
}
