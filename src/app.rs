use orx_parallel::*;
use std::error::Error;
use std::time::Instant;

use crate::IsochroneArgs;
use crate::isochrone::{self, IsochroneDisplayMode, compute_isochrones};
use chrono::Duration;
use geo::MultiPolygon;
use hrdf_parser::{Coordinates, Hrdf};
use isochrone::compute_optimal_isochrones;

#[cfg(feature = "hectare")]
use crate::{
    IsochroneHectareArgs,
    isochrone::externals::{HectareData, HectareRecord},
};

use self::isochrone::compute_average_isochrones;
use self::isochrone::compute_worst_isochrones;
use self::isochrone::utils::wgs84_to_lv95;

#[allow(clippy::too_many_arguments)]
pub fn run_simple(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args: IsochroneArgs,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args.time_limit.num_minutes();
    let isochrone_interval = isochrone_args.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args.latitude, isochrone_args.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    #[cfg(feature = "svg")]
    let iso = compute_isochrones(
        &hrdf,
        &excluded_polygons,
        isochrone_args,
        display_mode,
        num_threads,
    );

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
    num_threads: usize,
) -> Result<(), Box<dyn Error>> {
    let time_limit = isochrone_args.time_limit.num_minutes();
    let isochrone_interval = isochrone_args.interval.num_minutes();

    let (x, y) = wgs84_to_lv95(isochrone_args.latitude, isochrone_args.longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

    #[cfg(feature = "svg")]
    let iso = compute_average_isochrones(
        &hrdf,
        &excluded_polygons,
        isochrone_args,
        delta_time,
        num_threads,
    );

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

#[cfg(feature = "hectare")]
#[allow(clippy::too_many_arguments)]
pub fn run_surface_per_ha(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    hectare: HectareData,
    isochrone_args: IsochroneHectareArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> Result<Vec<HectareRecord>, Box<dyn Error>> {
    use std::sync::RwLock;

    let total_time = RwLock::new(Instant::now());
    let locked_counter = RwLock::new(0);
    let data = hectare.data();
    let total = data.len();
    let id_pos_surf = data
        .into_par()
        .num_threads(num_threads)
        .map(|record| {
            let start = Instant::now();
            let HectareRecord {
                reli,
                longitude,
                latitude,
                population,
                area,
            } = record;

            let verbose =  isochrone_args.verbose;
            if verbose {
                log::debug!("Computing max area for {reli} (longitude, latitude) = ({longitude}, {latitude}");
            }

            let he_re = if area.is_some() {
                record
            } else {
                use crate::utils::compute_remaining_threads;

                let IsochroneHectareArgs {
                    departure_at,
                    time_limit,
                    max_num_explorable_connections,
                    num_starting_points,
                    verbose,
                } = isochrone_args;
                let isochrone_args = IsochroneArgs {
                    latitude,
                    longitude,
                    departure_at,
                    time_limit,
                    interval: time_limit,
                    max_num_explorable_connections,
                    num_starting_points,
                    verbose,
                };
                let opt_iso = compute_optimal_isochrones(
                    &hrdf,
                    &excluded_polygons,
                    isochrone_args,
                    delta_time,
                    display_mode,
                    compute_remaining_threads(num_threads, total)
                );

                let area = opt_iso.compute_max_area();
                HectareRecord {
                    reli,
                    longitude,
                    latitude,
                    population,
                    area: Some(area),
                }
            };
            if verbose {
                let time = start.elapsed();
                {

                    let  elapsed = total_time.read().unwrap().elapsed();
                    {
                        let mut w = locked_counter.write().unwrap();
                        *w += 1;
                        let avg_time = elapsed / *w;
                        let remaining_time = avg_time * (total as i32 - *w as i32) as u32;
                        let remaining_time = Duration::from_std(remaining_time).expect("Unable to convert to a duration.");
                        let remaining_minutes = remaining_time - Duration::hours(remaining_time.num_hours());
                        let remaining_seconds = remaining_minutes - Duration::minutes(remaining_minutes.num_minutes());
                        log::debug!(
                            "Isochrone done for {reli} (longitude, latitude) = ({longitude}, {latitude}) in {time:.2?}"
                        );
                        log::info!(
                            "{w} / {total} done in {time:.2?}. Remaining {}h:{}m:{}s. Avg time per isochrone: {avg_time:.2?}.",
                            remaining_time.num_hours(), remaining_minutes.num_minutes(), remaining_seconds.num_seconds()
                        );
                    }
                }
            }
            he_re
        })
        .collect();

    Ok(id_pos_surf)
}

#[allow(clippy::too_many_arguments)]
pub fn run_optimal(
    hrdf: Hrdf,
    excluded_polygons: MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
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
        num_threads,
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
    num_threads: usize,
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
        num_threads,
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
    num_threads: usize,
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
        num_threads,
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
        num_threads,
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
