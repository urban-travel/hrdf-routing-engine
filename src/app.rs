//! The isochrone commands of the command line: compute, then draw into an SVG file.

use chrono::Duration;
use geo::MultiPolygon;
use hrdf_parser::{CoordinateSystem, Coordinates};

use crate::{
    IsochroneArgs, IsochroneDisplayMode, RResult, Router,
    isochrone::{
        IsochroneMap, compute_average_isochrones, compute_isochrones, compute_optimal_isochrones,
        compute_worst_isochrones, utils::wgs84_to_lv95,
    },
};

/// Which isochrones of the time window around `IsochroneArgs::at`.
#[derive(Clone, Copy)]
pub enum Pick {
    /// Those of `at` only.
    Simple,
    /// Those with the largest area.
    Optimal,
    /// Those with the smallest area.
    Worst,
    /// Those of the average time to each point.
    Average,
}

/// Draws the picked isochrones into `[reverse_][optimal_|worst_|average_]isochrones_<time
/// limit>_<interval>[_<delta time, for average>].svg`.
pub fn run_isochrones(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    args: &IsochroneArgs,
    pick: Pick,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> RResult<()> {
    let (map, name) = match pick {
        Pick::Simple => (
            compute_isochrones(router, excluded_polygons, args, display_mode, num_threads),
            "",
        ),
        Pick::Optimal => {
            let map = compute_optimal_isochrones(
                router,
                excluded_polygons,
                args,
                delta_time,
                display_mode,
                num_threads,
            );
            (map, "optimal_")
        }
        Pick::Worst => {
            let map = compute_worst_isochrones(
                router,
                excluded_polygons,
                args,
                delta_time,
                display_mode,
                num_threads,
            );
            (map, "worst_")
        }
        Pick::Average => (
            compute_average_isochrones(router, excluded_polygons, args, delta_time, num_threads),
            "average_",
        ),
    };
    let reverse = if args.reverse { "reverse_" } else { "" };
    let (time_limit, interval) = (args.time_limit.num_minutes(), args.interval.num_minutes());
    let delta = if let Pick::Average = pick {
        format!("_{}", delta_time.num_minutes())
    } else {
        String::new()
    };
    write_svg(
        &map,
        &format!("{reverse}{name}isochrones_{time_limit}_{interval}{delta}.svg"),
        args,
    )
}

/// The optimal isochrones at two dates, each drawn into `isochrones_<date>_<time limit>_<interval>.svg`,
/// with their area and reach.
#[allow(clippy::too_many_arguments)]
pub fn run_comparison(
    old_router: &Router,
    new_router: &Router,
    excluded_polygons: &MultiPolygon,
    old: &IsochroneArgs,
    new: &IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> RResult<()> {
    for (router, args) in [(old_router, old), (new_router, new)] {
        let map = compute_optimal_isochrones(
            router,
            excluded_polygons,
            args,
            delta_time,
            display_mode,
            num_threads,
        );
        let (time_limit, interval) = (args.time_limit.num_minutes(), args.interval.num_minutes());
        write_svg(
            &map,
            &format!("isochrones_{}_{time_limit}_{interval}.svg", args.at.date()),
            args,
        )?;
        let reach = map.compute_max_distance(center(args)).1;
        println!(
            "time = {}, surface = {}, max_distance = {reach}",
            map.departure_at(),
            map.compute_max_area()
        );
    }
    Ok(())
}

/// The point of the isochrones, in LV95.
fn center(args: &IsochroneArgs) -> Coordinates {
    let (easting, northing) = wgs84_to_lv95(args.latitude, args.longitude);
    Coordinates::new(CoordinateSystem::LV95, easting, northing)
}

fn write_svg(map: &IsochroneMap, path: &str, args: &IsochroneArgs) -> RResult<()> {
    #[cfg(feature = "svg")]
    map.write_svg(path, 1.0 / 100.0, Some(center(args)))?;
    #[cfg(not(feature = "svg"))]
    let _ = (map, path, args);
    Ok(())
}

/// The area of the optimal isochrone of every hectare not computed yet.
#[cfg(feature = "hectare")]
pub fn run_surface_per_ha(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    records: Vec<crate::HectareRecord>,
    args: &crate::cli::HectareArgs,
    num_threads: usize,
) -> Vec<crate::HectareRecord> {
    use orx_parallel::*;
    use std::{
        sync::atomic::{AtomicU32, Ordering},
        time::Instant,
    };

    let (started, done, total) = (Instant::now(), AtomicU32::new(0), records.len() as u32);
    let area = |latitude, longitude| {
        let isochrone_args = IsochroneArgs {
            latitude,
            longitude,
            at: args.departure_at,
            time_limit: args.time_limit,
            interval: args.time_limit,
            max_num_explorable_connections: args.max_num_explorable_connections,
            num_starting_points: args.num_starting_points,
            verbose: false,
            reverse: false,
        };
        let threads = crate::isochrone::inner_threads(num_threads);
        let map = compute_optimal_isochrones(
            router,
            excluded_polygons,
            &isochrone_args,
            args.delta_time,
            IsochroneDisplayMode::Circles,
            threads,
        );
        map.compute_max_area()
    };
    records
        .into_par()
        .num_threads(num_threads)
        .map(|record| {
            let area = record
                .area
                .unwrap_or_else(|| area(record.latitude, record.longitude));
            if args.verbose {
                let done = done.fetch_add(1, Ordering::Relaxed) + 1;
                let left = started.elapsed() / done * (total - done);
                log::info!(
                    "Hectare {} done ({done} / {total}, about {left:.0?} left)",
                    record.reli
                );
            }
            crate::HectareRecord {
                area: Some(area),
                ..record
            }
        })
        .collect()
}
