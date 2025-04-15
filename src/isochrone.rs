mod circles;
mod constants;
mod contour_line;
mod models;
pub mod utils;

use std::collections::HashMap;
use std::time::Instant;

use crate::isochrone::utils::haversine_distance;
use crate::routing::Route;
use crate::routing::compute_routes_from_origin;
use constants::WALKING_SPEED_IN_KILOMETERS_PER_HOUR;
use hrdf_parser::{CoordinateSystem, Coordinates, DataStorage, Hrdf, Model, Stop};
pub use models::DisplayMode as IsochroneDisplayMode;
pub use models::IsochroneMap;

use chrono::{Duration, NaiveDateTime};

use models::Isochrone;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use utils::lv95_to_wgs84;
use utils::time_to_distance;

use self::utils::NaiveDateTimeRange;
use self::utils::wgs84_to_lv95;

/// Computes the best isochrone in [departure_at - delta_time; departure_at + delta_time)
/// Best is defined by the maximal surface covered by the largest isochrone
#[allow(clippy::too_many_arguments)]
pub fn compute_optimal_isochrones(
    hrdf: &Hrdf,
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    display_mode: models::DisplayMode,
    verbose: bool,
) -> IsochroneMap {
    if verbose {
        log::info!(
            "longitude: {longitude}, latitude: {latitude}, departure_at: {departure_at}, time_limit: {}, isochrone_interval: {}, delta_time: {}, display_mode: {display_mode:?}, verbose: {verbose}",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes(),
            delta_time.num_minutes(),
        );
    }
    let start_time = Instant::now();
    let min_date_time = departure_at - delta_time;
    let max_date_time = departure_at + delta_time;

    let (isochrone_map, _) = NaiveDateTimeRange::new(
        min_date_time + Duration::minutes(1),
        max_date_time,
        Duration::minutes(1),
    )
    .into_iter()
    .collect::<Vec<_>>()
    .par_iter()
    .fold(
        || (IsochroneMap::default(), f64::MIN),
        |(iso_max, area_max), dep| {
            let isochrone = compute_isochrones(
                hrdf,
                longitude,
                latitude,
                *dep,
                time_limit,
                isochrone_interval,
                display_mode,
                false,
            );
            let curr_area = isochrone.compute_max_area();
            if curr_area > area_max {
                (isochrone, curr_area)
            } else {
                (iso_max, area_max)
            }
        },
    )
    .reduce(
        || (IsochroneMap::default(), f64::MIN),
        |(iso_max, area_max), (iso, area)| {
            if area > area_max {
                (iso, area)
            } else {
                (iso_max, area_max)
            }
        },
    );

    if verbose {
        log::info!(
            "Time computing the optimal solution : {:.2?}",
            start_time.elapsed()
        );
    }
    isochrone_map
}

/// Computes the worst isochrone in [departure_at - delta_time; departure_at + delta_time)
/// Best is defined by the maximal surface covered by the largest isochrone
#[allow(clippy::too_many_arguments)]
pub fn compute_worst_isochrones(
    hrdf: &Hrdf,
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    display_mode: models::DisplayMode,
    verbose: bool,
) -> IsochroneMap {
    if verbose {
        log::info!(
            "longitude: {longitude}, latitude: {latitude}, departure_at: {departure_at}, time_limit: {}, isochrone_interval: {}, delta_time: {}, display_mode: {display_mode:?}, verbose: {verbose}",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes(),
            delta_time.num_minutes(),
        );
    }
    let start_time = Instant::now();
    let min_date_time = departure_at - delta_time;
    let max_date_time = departure_at + delta_time;

    let (isochrone_map, _) = NaiveDateTimeRange::new(
        min_date_time + Duration::minutes(1),
        max_date_time,
        Duration::minutes(1),
    )
    .into_iter()
    .collect::<Vec<_>>()
    .par_iter()
    .fold(
        || (IsochroneMap::default(), f64::MAX),
        |(iso_max, area_max), dep| {
            let isochrone = compute_isochrones(
                hrdf,
                longitude,
                latitude,
                *dep,
                time_limit,
                isochrone_interval,
                display_mode,
                false,
            );
            let curr_area = isochrone.compute_max_area();
            if curr_area < area_max {
                (isochrone, curr_area)
            } else {
                (iso_max, area_max)
            }
        },
    )
    .reduce(
        || (IsochroneMap::default(), f64::MAX),
        |(iso_max, area_max), (iso, area)| {
            if area < area_max {
                (iso, area)
            } else {
                (iso_max, area_max)
            }
        },
    );

    if verbose {
        log::info!(
            "Time computing the optimal solution : {:.2?}",
            start_time.elapsed()
        );
    }
    isochrone_map
}

/// Computes the average isochrone.
/// The point of origin is used to find the departure stop (the nearest stop).
/// The departure date and time must be within the timetable period.
#[allow(clippy::too_many_arguments)]
pub fn compute_average_isochrones(
    hrdf: &Hrdf,
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    delta_time: Duration,
    verbose: bool,
) -> IsochroneMap {
    if verbose {
        log::info!(
            "Computing average isochrone:\n longitude: {longitude}, latitude: {latitude},  departure_at: {departure_at}, time_limit: {}, isochrone_interval: {}, delta_time: {}, verbose: {verbose}",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes(),
            delta_time.num_minutes()
        );
    }
    // If there is no departue stop found we just use the default
    let departure_coord = Coordinates::new(CoordinateSystem::WGS84, longitude, latitude);

    let (easting, northing) = wgs84_to_lv95(latitude, longitude);
    let departure_coord_lv95 = Coordinates::new(CoordinateSystem::LV95, easting, northing);

    let start_time = Instant::now();
    let min_date_time = departure_at - delta_time;
    let max_date_time = departure_at + delta_time;

    let data = NaiveDateTimeRange::new(
        min_date_time + Duration::minutes(1),
        max_date_time,
        Duration::minutes(1),
    )
    .into_iter()
    .collect::<Vec<_>>()
    .par_iter()
    .map(|dep| {
        let routes =
            compute_routes_from_origin(hrdf, latitude, longitude, *dep, time_limit, 5, verbose);

        unique_coordinates_from_routes(&routes, departure_at)
    })
    .collect::<Vec<_>>();
    let bounding_box = data.iter().fold(
        ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)),
        |cover_bb, d| {
            let bb = get_bounding_box(d, time_limit);
            let x0 = f64::min(cover_bb.0.0, bb.0.0);
            let x1 = f64::max(cover_bb.1.0, bb.1.0);
            let y0 = f64::min(cover_bb.0.1, bb.0.1);
            let y1 = f64::max(cover_bb.1.1, bb.1.1);
            ((x0, y0), (x1, y1))
        },
    );

    let num_points = 1500;
    let mut grids = data
        .into_iter()
        .map(|d| contour_line::create_grid(&d, bounding_box, time_limit, num_points))
        .collect::<Vec<_>>();
    let timesteps = grids.len();
    let grid_ini = grids.pop().expect("Grids was empty");
    let (total_grid, nx, ny, dx) =
        grids
            .into_iter()
            .fold(grid_ini, |(total, nx, ny, dx), (g, _, _, _)| {
                let new_grid = g
                    .into_iter()
                    .zip(total)
                    .map(|((lc, ld), (_, rd))| (lc, (rd + ld)))
                    .collect::<Vec<_>>();
                (new_grid, nx, ny, dx)
            });
    let avg_grid = total_grid
        .into_iter()
        .map(|(c, d)| (c, d / timesteps as i32))
        .collect::<Vec<_>>();
    let isochrone_count = time_limit.num_minutes() / isochrone_interval.num_minutes();
    let isochrones = (0..isochrone_count)
        .map(|i| {
            let current_time_limit = Duration::minutes(isochrone_interval.num_minutes() * (i + 1));

            let polygons = contour_line::get_polygons(
                &avg_grid,
                nx,
                ny,
                bounding_box.0,
                current_time_limit,
                dx,
            );

            Isochrone::new(polygons, current_time_limit.num_minutes() as u32)
        })
        .collect::<Vec<_>>();

    let areas = isochrones.iter().map(|i| i.compute_area()).collect();
    let max_distances = isochrones
        .iter()
        .map(|i| {
            let ((x, y), max) = i.compute_max_distance(departure_coord_lv95);
            let (w_x, w_y) = lv95_to_wgs84(x, y);
            ((w_x, w_y), max)
        })
        .collect();

    if verbose {
        log::info!(
            "Time for finding the isochrones : {:.2?}",
            start_time.elapsed()
        );
    }
    IsochroneMap::new(
        isochrones,
        areas,
        max_distances,
        departure_coord,
        departure_at,
        convert_bounding_box_to_wgs84(bounding_box),
    )
}

/// Computes the isochrones.
/// The point of origin is used to find the departure stop (the nearest stop).
/// The departure date and time must be within the timetable period.
#[allow(clippy::too_many_arguments)]
pub fn compute_isochrones(
    hrdf: &Hrdf,
    longitude: f64,
    latitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    display_mode: models::DisplayMode,
    verbose: bool,
) -> IsochroneMap {
    if verbose {
        log::info!(
            "longitude: {longitude}, latitude : {latitude},  departure_at: {departure_at}, time_limit: {}, isochrone_interval: {}, display_mode: {display_mode:?}, verbose: {verbose}",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        );
    }
    // If there is no departue stop found we just use the default
    let departure_coord = Coordinates::new(CoordinateSystem::WGS84, longitude, latitude);

    let (easting, northing) = wgs84_to_lv95(latitude, longitude);
    let departure_coord_lv95 = Coordinates::new(CoordinateSystem::LV95, easting, northing);

    let start_time = Instant::now();

    let routes = compute_routes_from_origin(
        hrdf,
        latitude,
        longitude,
        departure_at,
        time_limit,
        5,
        verbose,
    );

    if verbose {
        log::info!("Time for finding the routes : {:.2?}", start_time.elapsed());
    }

    let start_time = Instant::now();

    // We get only the stop coordinates
    let data = unique_coordinates_from_routes(&routes, departure_at);

    let bounding_box = get_bounding_box(&data, time_limit);
    let num_points = 1500;

    let grid = if display_mode == models::DisplayMode::ContourLine {
        Some(contour_line::create_grid(
            &data,
            bounding_box,
            time_limit,
            num_points,
        ))
    } else {
        None
    };

    let isochrone_count = time_limit.num_minutes() / isochrone_interval.num_minutes();
    let isochrones = (0..isochrone_count)
        .map(|i| {
            let current_time_limit = Duration::minutes(isochrone_interval.num_minutes() * (i + 1));

            let polygons = match display_mode {
                IsochroneDisplayMode::Circles => circles::get_polygons(&data, current_time_limit),
                IsochroneDisplayMode::ContourLine => {
                    let (grid, num_points_x, num_points_y, dx) = grid.as_ref().unwrap();
                    contour_line::get_polygons(
                        grid,
                        *num_points_x,
                        *num_points_y,
                        bounding_box.0,
                        current_time_limit,
                        *dx,
                    )
                }
            };

            Isochrone::new(polygons, current_time_limit.num_minutes() as u32)
        })
        .collect::<Vec<_>>();

    let areas = isochrones.iter().map(|i| i.compute_area()).collect();
    let max_distances = isochrones
        .iter()
        .map(|i| {
            let ((x, y), max) = i.compute_max_distance(departure_coord_lv95);
            let (w_x, w_y) = lv95_to_wgs84(x, y);
            ((w_x, w_y), max)
        })
        .collect();

    if verbose {
        log::info!(
            "Time for finding the isochrones : {:.2?}",
            start_time.elapsed()
        );
    }
    IsochroneMap::new(
        isochrones,
        areas,
        max_distances,
        departure_coord,
        departure_at,
        convert_bounding_box_to_wgs84(bounding_box),
    )
}

#[allow(dead_code)]
fn find_nearest_stop(
    data_storage: &DataStorage,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
) -> &Stop {
    data_storage
        .stops()
        .entries()
        .into_iter()
        // Only considers stops in Switzerland.
        .filter(|stop| stop.id().to_string().starts_with("85"))
        .filter(|stop| stop.wgs84_coordinates().is_some())
        .min_by(|a, b| {
            let coord_1 = a.wgs84_coordinates().unwrap();
            let distance_1 = haversine_distance(
                origin_point_latitude,
                origin_point_longitude,
                coord_1.latitude().expect("Wrong coordinate system"),
                coord_1.longitude().expect("Wrong coordinate system"),
            );

            let coord_2 = b.wgs84_coordinates().unwrap();
            let distance_2 = haversine_distance(
                origin_point_latitude,
                origin_point_longitude,
                coord_2.latitude().expect("Wrong coordinate system"),
                coord_2.longitude().expect("Wrong coordinate system"),
            );

            distance_1.partial_cmp(&distance_2).unwrap()
        })
        // The stop list cannot be empty.
        .unwrap()
}

/// Each coordinate should be kept only once with the minimum duration associated
fn unique_coordinates_from_routes(
    routes: &[Route],
    departure_at: NaiveDateTime,
) -> Vec<(Coordinates, Duration)> {
    let mut coordinates_duration: HashMap<i32, (Coordinates, chrono::TimeDelta)> = HashMap::new();
    for route in routes {
        let arrival_stop = route.sections().last().expect("Route sections was empty");
        let arrival_stop_id = arrival_stop.arrival_stop_id();
        let arrival_stop_coords = if let Some(c) = arrival_stop.arrival_stop_lv95_coordinates() {
            c
        } else {
            continue;
        };
        let new_duration = route.arrival_at() - departure_at;
        if let Some((_, duration)) = coordinates_duration.get_mut(&arrival_stop_id) {
            // We want the shortest trip duration to be kept only
            if new_duration < *duration {
                *duration = new_duration;
            }
        } else {
            let _ =
                coordinates_duration.insert(arrival_stop_id, (arrival_stop_coords, new_duration));
        }
    }
    coordinates_duration.into_values().collect()
}

fn get_bounding_box(
    data: &[(Coordinates, Duration)],
    time_limit: Duration,
) -> ((f64, f64), (f64, f64)) {
    let min_x = data
        .iter()
        .fold(f64::INFINITY, |result, &(coord, duration)| {
            let candidate = coord.easting().expect("Wrong coordinate system")
                - time_to_distance(time_limit - duration, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
            f64::min(result, candidate)
        });

    let max_x = data
        .iter()
        .fold(f64::NEG_INFINITY, |result, &(coord, duration)| {
            let candidate = coord.easting().expect("Wrong coordinate system")
                + time_to_distance(time_limit - duration, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
            f64::max(result, candidate)
        });

    let min_y = data
        .iter()
        .fold(f64::INFINITY, |result, &(coord, duration)| {
            let candidate = coord.northing().expect("Wrong coordinate system")
                - time_to_distance(time_limit - duration, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
            f64::min(result, candidate)
        });

    let max_y = data
        .iter()
        .fold(f64::NEG_INFINITY, |result, &(coord, duration)| {
            let candidate = coord.northing().expect("Wrong coordinate system")
                + time_to_distance(time_limit - duration, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
            f64::max(result, candidate)
        });

    ((min_x, min_y), (max_x, max_y))
}

fn convert_bounding_box_to_wgs84(
    bounding_box: ((f64, f64), (f64, f64)),
) -> ((f64, f64), (f64, f64)) {
    (
        lv95_to_wgs84(bounding_box.0.0, bounding_box.0.1),
        lv95_to_wgs84(bounding_box.1.0, bounding_box.1.1),
    )
}
