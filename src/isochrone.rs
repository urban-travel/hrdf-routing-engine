mod circles;
mod constants;
mod contour_line;
pub(crate) mod externals;
mod models;
pub(crate) mod utils;

use std::collections::HashMap;
use std::fmt::Display;
use std::time::Instant;

use crate::isochrone::utils::haversine_distance;
use crate::routing::Route;
use crate::routing::compute_routes_from_origin;
use crate::utils::compute_remaining_threads;
use constants::WALKING_SPEED_IN_KILOMETERS_PER_HOUR;
use geo::BooleanOps;
use geo::MultiPolygon;
use hrdf_parser::{CoordinateSystem, Coordinates, DataStorage, Hrdf, Stop};
pub use models::DisplayMode as IsochroneDisplayMode;
pub use models::IsochroneMap;

use chrono::{Duration, NaiveDateTime};

use models::Isochrone;
use orx_parallel::*;
use utils::lv95_to_wgs84;
use utils::time_to_distance;

use self::utils::NaiveDateTimeRange;
use self::utils::wgs84_to_lv95;

#[cfg(feature = "hectare")]
#[derive(Clone, Debug)]
pub struct IsochroneHectareArgs {
    /// Departure date and time
    pub departure_at: NaiveDateTime,
    /// Maximum time of the isochrone in minutes
    pub time_limit: Duration,
    /// Maximum number of connections
    pub max_num_explorable_connections: i32,
    /// Number of starting points
    pub num_starting_points: usize,
    /// Verbose on or off
    pub verbose: bool,
}

#[derive(Debug, Clone)]
pub struct IsochroneArgs {
    pub latitude: f64,
    pub longitude: f64,
    pub departure_at: NaiveDateTime,
    pub time_limit: Duration,
    pub interval: Duration,
    pub max_num_explorable_connections: i32,
    pub num_starting_points: usize,
    pub verbose: bool,
}

impl Display for IsochroneArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "longitude: {}, latitude: {}, departure_at: {}, time_limit: {}, interval: {}",
            self.longitude, self.latitude, self.departure_at, self.time_limit, self.interval
        )
    }
}

/// Computes the best isochrone in [departure_at - delta_time; departure_at + delta_time)
/// Best is defined by the maximal surface covered by the largest isochrone
pub fn compute_optimal_isochrones(
    hrdf: &Hrdf,
    excluded_polygons: &MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
    display_mode: models::DisplayMode,
    num_threads: usize,
) -> IsochroneMap {
    let IsochroneArgs {
        latitude,
        longitude,
        departure_at,
        time_limit,
        interval: isochrone_interval,
        max_num_explorable_connections,
        num_starting_points,
        verbose,
    } = isochrone_args;

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

    let isochrone_map = NaiveDateTimeRange::new(
        min_date_time,
        max_date_time,
        Duration::minutes(1),
    )
    .into_iter()
    .collect::<Vec<_>>();
    let num_dates = isochrone_map.len();

    let isochrone_map = isochrone_map
        .into_par()
        .num_threads(num_threads)
        .map(|dep| {
            compute_isochrones(
                hrdf,
                excluded_polygons,
                IsochroneArgs {
                    latitude,
                    longitude,
                    departure_at: dep,
                    time_limit,
                    interval: isochrone_interval,
                    max_num_explorable_connections,
                    num_starting_points,
                    verbose,
                },
                display_mode,
                compute_remaining_threads(num_threads, num_dates),
            )
        })
        .reduce(|lhs, rhs| {
            if lhs.compute_max_area() > rhs.compute_max_area() {
                lhs
            } else {
                rhs
            }
        });

    if verbose {
        log::info!(
            "Time computing the optimal solution : {:.2?}",
            start_time.elapsed()
        );
    }
    isochrone_map.expect("No isochrone_map found.")
}

/// Computes the worst isochrone in [departure_at - delta_time; departure_at + delta_time)
/// Best is defined by the maximal surface covered by the largest isochrone
#[allow(clippy::too_many_arguments)]
pub fn compute_worst_isochrones(
    hrdf: &Hrdf,
    excluded_polygons: &MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
    display_mode: models::DisplayMode,
    num_threads: usize,
) -> IsochroneMap {
    let IsochroneArgs {
        latitude,
        longitude,
        departure_at,
        time_limit,
        interval: isochrone_interval,
        max_num_explorable_connections,
        num_starting_points,
        verbose,
    } = isochrone_args;

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

    let isochrone_map = NaiveDateTimeRange::new(
        min_date_time,
        max_date_time,
        Duration::minutes(1),
    )
    .into_iter()
    .collect::<Vec<_>>();
    let total_dates = isochrone_map.len();

    let isochrone_map = isochrone_map
        .into_par()
        .num_threads(num_threads)
        .map(|dep| {
            compute_isochrones(
                hrdf,
                excluded_polygons,
                IsochroneArgs {
                    latitude,
                    longitude,
                    departure_at: dep,
                    time_limit,
                    interval: isochrone_interval,
                    max_num_explorable_connections,
                    num_starting_points,
                    verbose,
                },
                display_mode,
                compute_remaining_threads(num_threads, total_dates),
            )
        })
        .reduce(|lhs, rhs| {
            if lhs.compute_max_area() < rhs.compute_max_area() {
                lhs
            } else {
                rhs
            }
        });

    if verbose {
        log::info!(
            "Time computing the optimal solution : {:.2?}",
            start_time.elapsed()
        );
    }
    isochrone_map.expect("Could not find worst Isochrone Map")
}

/// Computes the average isochrone.
/// The point of origin is used to find the departure stop (the nearest stop).
/// The departure date and time must be within the timetable period.
#[allow(clippy::too_many_arguments)]
pub fn compute_average_isochrones(
    hrdf: &Hrdf,
    excluded_polygons: &MultiPolygon,
    isochrone_args: IsochroneArgs,
    delta_time: Duration,
    num_threads: usize,
) -> IsochroneMap {
    let IsochroneArgs {
        latitude,
        longitude,
        departure_at,
        time_limit,
        interval: isochrone_interval,
        max_num_explorable_connections,
        num_starting_points,
        verbose,
    } = isochrone_args;

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
        min_date_time,
        max_date_time,
        Duration::minutes(1),
    )
    .into_iter()
    .collect::<Vec<_>>();

    let num_dates = data.len();

    let data = data
        .par()
        .num_threads(num_threads)
        .map(|dep| {
            let routes = compute_routes_from_origin(
                hrdf,
                latitude,
                longitude,
                *dep,
                time_limit,
                num_starting_points,
                compute_remaining_threads(num_threads, num_dates),
                max_num_explorable_connections,
                verbose,
            );

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

    let dx = 100.0;
    let mut grids = data
        .into_iter()
        .map(|d| contour_line::create_grid(&d, bounding_box, time_limit, dx, num_threads))
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

            let polygons = MultiPolygon(polygons.into_iter().collect());
            let polygons = polygons.difference(excluded_polygons);
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
    excluded_polygons: &MultiPolygon,
    isochrone_args: IsochroneArgs,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> IsochroneMap {
    let IsochroneArgs {
        latitude,
        longitude,
        departure_at,
        time_limit,
        interval: isochrone_interval,
        max_num_explorable_connections,
        num_starting_points,
        verbose,
    } = isochrone_args;

    if verbose {
        log::info!(
            "longitude: {longitude}, latitude : {latitude},  departure_at: {departure_at}, time_limit: {}, isochrone_interval: {}, display_mode: {display_mode:?}, max_num_explorable_connections: {max_num_explorable_connections}, verbose: {verbose}",
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
        num_starting_points,
        num_threads,
        max_num_explorable_connections,
        verbose,
    );

    if verbose {
        log::info!("Time for finding the routes : {:.2?}", start_time.elapsed());
    }

    let start_time = Instant::now();

    // We get only the stop coordinates
    let data = unique_coordinates_from_routes(&routes, departure_at);

    let bounding_box = get_bounding_box(&data, time_limit);
    let dx = 100.0;

    let grid = if display_mode == models::DisplayMode::ContourLine {
        Some(contour_line::create_grid(
            &data,
            bounding_box,
            time_limit,
            dx,
            num_threads,
        ))
    } else {
        None
    };

    let isochrone_count = time_limit.num_minutes() / isochrone_interval.num_minutes();
    let isochrones = (0..isochrone_count)
        .map(|i| {
            let current_time_limit = Duration::minutes(isochrone_interval.num_minutes() * (i + 1));
            // let prev_time_limit = Duration::minutes(isochrone_interval.num_minutes() * i);
            let prev_time_limit = Duration::minutes(0);

            let polygons = match display_mode {
                IsochroneDisplayMode::Circles => {
                    let num_points_circle = 6;
                    circles::get_polygons(
                        &data,
                        current_time_limit,
                        prev_time_limit,
                        num_points_circle,
                        num_threads,
                    )
                }
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
            // let polygons = MultiPolygon(polygons.into_iter().collect());
            let polygons = polygons.difference(excluded_polygons);

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
pub(crate) fn unique_coordinates_from_routes(
    routes: &[Route],
    departure_at: NaiveDateTime,
) -> Vec<(Coordinates, Duration)> {
    let mut coordinates_duration: HashMap<i32, (Coordinates, chrono::Duration)> = HashMap::new();
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

#[cfg(test)]
mod polygon_tests {
    use super::*;
    use crate::routing::compute_routes_from_origin;
    use crate::utils::create_date_time;
    use geo::{Area, Contains, Point};
    use std::f64::consts::PI;
    use std::fs::File;
    use std::io::Write;
    use test_log::test;

    /// Helper function that computes the area of an hexagon
    fn hexagon_area(r: f64) -> f64 {
        (3.0 * 3.0_f64.sqrt() / 2.0) * r * r
    }

    /// Helper to calculate area in square meters (converts WGS84 to LV95 first)
    fn calculate_area_m2(polygon: &MultiPolygon) -> f64 {
        utils::multi_polygon_to_lv95(polygon)
            .iter()
            .map(|p| p.unsigned_area())
            .sum()
    }

    /// Helper to save polygon as GeoJSON for visual inspection
    #[allow(dead_code)]
    fn save_geojson(path: &str, polygon: &MultiPolygon) -> std::io::Result<()> {
        use serde_json::json;

        let features = polygon
            .0
            .iter()
            .map(|poly| {
                let coords: Vec<Vec<[f64; 2]>> =
                    vec![poly.exterior().coords().map(|c| [c.x, c.y]).collect()];

                json!({
                    "type": "Feature",
                    "properties": {},
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": coords
                    }
                })
            })
            .collect::<Vec<_>>();

        let geojson = json!({
            "type": "FeatureCollection",
            "features": features
        });

        let mut file = File::create(path)?;
        file.write_all(serde_json::to_string_pretty(&geojson)?.as_bytes())?;
        Ok(())
    }

    #[test]
    fn test_circles_polygon_geometry_single_point() {
        // Test data: single point with 30 min remaining time
        let data = vec![(
            Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0),
            Duration::minutes(0),
        )];

        // The polygon is an hexagon (6 argument)
        let result =
            circles::get_polygons(&data, Duration::minutes(30), Duration::minutes(0), 6, 1);

        // Basic assertions
        assert!(!result.0.is_empty(), "Result should not be empty");
        assert_eq!(result.0.len(), 1, "Should have exactly one polygon");

        // Check area matches analytical formula for regular hexagon
        let area = calculate_area_m2(&result);
        let radius = time_to_distance(Duration::minutes(30), WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
        let expected_area = hexagon_area(radius);

        let tolerance = 0.0001;
        let diff = (area - expected_area).abs() / expected_area;
        assert!(
            diff < tolerance,
            "Area difference too large: {:.2}% (got {:.2}, expected {:.2})",
            diff * 100.0,
            area,
            expected_area
        );
    }

    #[test]
    fn test_circles_polygon_geometry_multiple_points() {
        let data = vec![
            (
                Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0),
                Duration::minutes(10),
            ),
            (
                Coordinates::new(CoordinateSystem::LV95, 2601000.0, 1200000.0),
                Duration::minutes(20),
            ),
            (
                Coordinates::new(CoordinateSystem::LV95, 2600500.0, 1200500.0),
                Duration::minutes(15),
            ),
        ];

        let result =
            circles::get_polygons(&data, Duration::minutes(30), Duration::minutes(0), 6, 1);
        assert!(!result.0.is_empty(), "Result should not be empty");
        assert_eq!(result.0.len(), 1, "Should be unioned into single polygon");

        let area = calculate_area_m2(&result);
        let radius = time_to_distance(Duration::minutes(30), WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
        let expected_max_area = 3.0 * hexagon_area(radius);
        assert!(area > 0.0, "Area should be positive");
        assert!(
            area < expected_max_area,
            "Area should be smaller than 3 times 30minutes hexagons"
        );
    }

    #[test]
    fn test_circles_polygon_contains_reachable_points() {
        let data = vec![
            (
                Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0),
                Duration::minutes(10),
            ),
            (
                Coordinates::new(CoordinateSystem::LV95, 2601000.0, 1200000.0),
                Duration::minutes(20),
            ),
        ];

        let result =
            circles::get_polygons(&data, Duration::minutes(30), Duration::minutes(0), 6, 1);

        for (coord_lv95, duration) in &data {
            assert!(
                *duration <= Duration::minutes(30),
                "Duration should be smaller than 30min"
            );
            let (lat, lon) = lv95_to_wgs84(
                coord_lv95.easting().unwrap(),
                coord_lv95.northing().unwrap(),
            );
            let point = Point::new(lat, lon);

            assert!(
                result.contains(&point),
                "Polygon should contain reachable point at ({}, {})",
                lat,
                lon
            );
        }
    }

    #[test]
    fn test_circles_edge_case_time_filter() {
        let data = vec![
            (
                Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0),
                Duration::minutes(5),
            ),
            (
                Coordinates::new(CoordinateSystem::LV95, 2610000.0, 1200000.0),
                Duration::minutes(35),
            ), // Filtered out
            (
                Coordinates::new(CoordinateSystem::LV95, 2600500.0, 1200500.0),
                Duration::minutes(15),
            ),
        ];

        let result =
            circles::get_polygons(&data, Duration::minutes(30), Duration::minutes(0), 6, 1);

        // Should only include first and third points
        assert!(!result.0.is_empty());

        // Check the 35-minute point is NOT contained (it should be filtered)
        let (lat, lon) = lv95_to_wgs84(2610000.0, 1200000.0);
        let point = Point::new(lat, lon);
        assert!(
            !result.contains(&point),
            "Polygon should not contain reachable point at ({}, {})",
            lat,
            lon
        );
    }

    #[test]
    fn test_circles_edge_case_overlapping_circles() {
        // Many overlapping circles (stress test union operation)
        let clustered: Vec<_> = (0..50)
            .map(|i| {
                let offset = i as f64 * 100.0; // 100m apart
                (
                    Coordinates::new(CoordinateSystem::LV95, 2600000.0 + offset, 1200000.0),
                    Duration::minutes(5),
                )
            })
            .collect();

        let result = circles::get_polygons(
            &clustered,
            Duration::minutes(30),
            Duration::minutes(0),
            6,
            1,
        );

        assert!(!result.0.is_empty());
        assert_eq!(result.0.len(), 1, "Should union into single polygon");

        let area = calculate_area_m2(&result);
        let expected_max_area = 50.0
            * hexagon_area(time_to_distance(
                Duration::minutes(25),
                WALKING_SPEED_IN_KILOMETERS_PER_HOUR,
            ));

        assert!(area > 0.0, "Area must be positive but was {}", area);
        assert!(
            area < expected_max_area,
            "The area should be smaller than the area of 50, we got area: {}, max_area: {}",
            area,
            expected_max_area
        );
    }

    #[test]
    fn test_contour_polygon_geometry() {
        // Create sample grid data
        let ce = 2600000.0;
        let cn = 1200000.0;
        let walking_speed_m_per_min = WALKING_SPEED_IN_KILOMETERS_PER_HOUR * 1000.0 / 60.0;
        let data: Vec<_> = (-50..50)
            .flat_map(|i| {
                (-50..50).map(move |j| {
                    let e = ce + i as f64 * 10.0;
                    let n = cn + j as f64 * 10.0;
                    let dist = (e - ce) * (e - ce) + (n - cn) * (n - cn);
                    let duration =
                        Duration::minutes((dist.sqrt() / walking_speed_m_per_min) as i64);
                    (Coordinates::new(CoordinateSystem::LV95, e, n), duration)
                })
            })
            .collect();

        let bbox = get_bounding_box(&data, Duration::minutes(10));
        let dx = 100.0;

        let (grid, nx, ny, dx) =
            contour_line::create_grid(&data, bbox, Duration::minutes(10), dx, 1);

        let result = contour_line::get_polygons(&grid, nx, ny, bbox.0, Duration::minutes(5), dx);

        assert!(!result.0.is_empty(), "Contour result should not be empty");
        let area = calculate_area_m2(&result);
        let max_six_min_area = PI
            * time_to_distance(Duration::minutes(6), WALKING_SPEED_IN_KILOMETERS_PER_HOUR)
            * time_to_distance(Duration::minutes(6), WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
        assert!(area > 0.0, "Contour area should be positive");
        assert!(
            area < max_six_min_area,
            "Contour area should certainly be smaller than the 6 min circle, but we have: area = {}, circle = {}",
            area,
            max_six_min_area
        );
        let max_four_min_area = PI
            * time_to_distance(Duration::minutes(4), WALKING_SPEED_IN_KILOMETERS_PER_HOUR)
            * time_to_distance(Duration::minutes(4), WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
        assert!(
            area > max_four_min_area,
            "Contour area should certainly be larger than the 4 min circle, but we have: area = {}, circle = {}",
            area,
            max_four_min_area
        );
    }

    #[test(tokio::test)]
    async fn test_polygon_generation_performance() {
        let hrdf = Hrdf::try_from_year(2025, true, None).await.unwrap();
        let departure_at = create_date_time(2025, 6, 15, 12, 10);

        let routes = compute_routes_from_origin(
            &hrdf,
            47.3769,
            8.5417, // Zürich
            departure_at,
            Duration::minutes(60),
            10,
            8,
            10,
            false,
        );

        let data = unique_coordinates_from_routes(&routes, departure_at);

        log::info!("Performance test with {} data points", data.len());

        // Measure circles mode
        let start = Instant::now();
        let circles_result =
            circles::get_polygons(&data, Duration::minutes(60), Duration::minutes(0), 24, 1);
        let circles_time = start.elapsed();
        log::info!(
            "Time for computation of circles polygon: {:?}",
            circles_time
        );

        // Measure contour mode
        let bbox = get_bounding_box(&data, Duration::minutes(60));
        let (grid, nx, ny, dx) =
            contour_line::create_grid(&data, bbox, Duration::minutes(60), 50.0, 1);

        let start = Instant::now();
        let contour_result =
            contour_line::get_polygons(&grid, nx, ny, bbox.0, Duration::minutes(60), dx);
        let contour_time = start.elapsed();
        log::info!(
            "Time for computation of contour polygon: {:?}",
            contour_time
        );

        assert!(!circles_result.0.is_empty());
        assert!(!contour_result.0.is_empty());
        let area_circles = calculate_area_m2(&circles_result);
        let area_contour = calculate_area_m2(&contour_result);
        assert!(
            area_circles > 0.0 && area_contour > 0.0,
            "Areas should be positive, but area_circles: {}, and area_contour: {}",
            area_circles,
            area_contour
        );
        let thres = 0.01;
        let diff = (area_circles - area_contour).abs() / area_contour;
        assert!(
            diff < thres,
            "We expect smaller than 1% difference between the 2 but, area_circles: {}, area_contour: {}, diff: {}",
            area_circles,
            area_contour,
            diff
        );
    }

    #[test]
    fn test_polygon_area_bounds_single_point() {
        // For a single point with 30 min time limit and 5 km/h walking speed:
        // Maximum walkable distance = 30/60 * 5 = 2.5 km
        // Expected area ≈ π * (2500)^2 ≈ 19.6 million m²

        let single_point = vec![(
            Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0),
            Duration::minutes(0),
        )];

        let result = circles::get_polygons(
            &single_point,
            Duration::minutes(30),
            Duration::minutes(0),
            36, // More points for better circle approximation
            1,
        );

        let area_m2 = calculate_area_m2(&result);
        let distance =
            time_to_distance(Duration::minutes(30), WALKING_SPEED_IN_KILOMETERS_PER_HOUR);
        let expected_area = std::f64::consts::PI * distance * distance;

        // Allow 1% tolerance for polygon approximation (more points = more accurate)
        let tolerance = 0.01;
        let diff = (area_m2 - expected_area).abs() / expected_area;
        assert!(
            diff < tolerance,
            "Area difference too large: {:.2}% (got {:.2}, expected {:.2})",
            diff * 100.0,
            area_m2,
            expected_area
        );
    }

    #[test]
    fn test_polygon_area_increases_with_time() {
        // Create a single point but generate polygons for different time limits
        let data = vec![(
            Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0),
            Duration::minutes(0),
        )];

        let poly_20 =
            circles::get_polygons(&data, Duration::minutes(20), Duration::minutes(0), 6, 1);
        let poly_30 =
            circles::get_polygons(&data, Duration::minutes(30), Duration::minutes(0), 6, 1);
        let poly_40 =
            circles::get_polygons(&data, Duration::minutes(40), Duration::minutes(0), 6, 1);

        let area_20 = calculate_area_m2(&poly_20);
        let area_30 = calculate_area_m2(&poly_30);
        let area_40 = calculate_area_m2(&poly_40);

        assert!(
            area_30 > area_20,
            "Area should increase with time limit, but we have area_20: {}, area_30: {}",
            area_20,
            area_30
        );
        assert!(
            area_40 > area_30,
            "Area should increase with time limit, but we have area_30: {}, area_40: {}",
            area_30,
            area_40
        );
    }
}
