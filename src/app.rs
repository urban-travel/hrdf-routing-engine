use std::error::Error;

use crate::isochrone::{self, IsochroneDisplayMode, compute_isochrones};
use chrono::{Duration, NaiveDateTime};
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
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    display_mode: IsochroneDisplayMode,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let (x, y) = wgs84_to_lv95(latitude, longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    #[cfg(feature = "svg")]
    let iso = compute_isochrones(
        &hrdf,
        &excluded_polygons,
        longitude,
        latitude,
        departure_at,
        time_limit,
        isochrone_interval,
        display_mode,
        verbose,
    );

    #[cfg(feature = "svg")]
    iso.write_svg(
        &format!(
            "isochrones_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_average(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let (x, y) = wgs84_to_lv95(latitude, longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    #[cfg(feature = "svg")]
    let iso = compute_average_isochrones(
        &hrdf,
        &excluded_polygons,
        longitude,
        latitude,
        departure_at,
        time_limit,
        isochrone_interval,
        delta_time,
        verbose,
    );

    #[cfg(feature = "svg")]
    iso.write_svg(
        &format!(
            "average_isochrones_{}_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes(),
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
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let (x, y) = wgs84_to_lv95(latitude, longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    let opt_iso = compute_optimal_isochrones(
        &hrdf,
        &excluded_polygons,
        longitude,
        latitude,
        departure_at,
        time_limit,
        isochrone_interval,
        delta_time,
        display_mode,
        verbose,
    );

    #[cfg(feature = "svg")]
    opt_iso.write_svg(
        &format!(
            "optimal_isochrones_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_worst(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let (x, y) = wgs84_to_lv95(latitude, longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    let opt_iso = compute_worst_isochrones(
        &hrdf,
        &excluded_polygons,
        longitude,
        latitude,
        departure_at,
        time_limit,
        isochrone_interval,
        delta_time,
        display_mode,
        verbose,
    );

    #[cfg(feature = "svg")]
    opt_iso.write_svg(
        &format!(
            "worst_isochrones_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
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
    longitude: f64,
    latitude: f64,
    departure_at_2024: NaiveDateTime,
    departure_at_2025: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let (easting, northing) = wgs84_to_lv95(latitude, longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, easting, northing);

    let isochrones_2024 = compute_optimal_isochrones(
        &hrdf_2024,
        &excluded_polygons,
        longitude,
        latitude,
        departure_at_2024,
        time_limit,
        isochrone_interval,
        delta_time,
        display_mode,
        verbose,
    );
    #[cfg(feature = "svg")]
    isochrones_2024.write_svg(
        &format!(
            "isochrones_2024_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
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
        longitude,
        latitude,
        departure_at_2025,
        time_limit,
        isochrone_interval,
        delta_time,
        display_mode,
        verbose,
    );
    #[cfg(feature = "svg")]
    isochrones_2025.write_svg(
        &format!(
            "isochrones_2025_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
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
