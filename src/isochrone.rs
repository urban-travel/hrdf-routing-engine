mod circles;
mod constants;
mod contour_line;
mod models;
mod utils;

use std::time::Instant;

use crate::isochrone::utils::haversine_distance;
use crate::routing::find_reachable_stops_within_time_limit;
use crate::routing::Route;
use crate::routing::RouteSection;
use constants::WALKING_SPEED_IN_KILOMETERS_PER_HOUR;
use hrdf_parser::CoordinateSystem;
use hrdf_parser::Coordinates;
use hrdf_parser::DataStorage;
use hrdf_parser::Hrdf;
use hrdf_parser::Model;
use hrdf_parser::Stop;
pub use models::DisplayMode as IsochroneDisplayMode;
pub use models::IsochroneMap;

use chrono::{Duration, NaiveDateTime};

use models::Isochrone;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use utils::distance_to_time;
use utils::lv95_to_wgs84;
use utils::time_to_distance;
use utils::wgs84_to_lv95;

/// Computes the isochrones.
/// The point of origin is used to find the departure stop (the nearest stop).
/// The departure date and time must be within the timetable period.
#[allow(clippy::too_many_arguments)]
pub fn compute_isochrones(
    hrdf: &Hrdf,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    isochrone_interval: Duration,
    display_mode: models::DisplayMode,
    verbose: bool,
) -> IsochroneMap {
    log::info!("origin_point_latitude : {origin_point_latitude}, origin_point_longitude: {origin_point_longitude}, departure_at: {departure_at}, time_limit: {time_limit}, isochrone_interval: {isochrone_interval}, display_mode: {display_mode:?}, verbose: {verbose}");
    let start_time = Instant::now();

    // Create a list of stops close enough to be of interest
    // We limit ourselves to the 10 closest. It may not be the best choice but otherwise the
    // computation becomes very slow due to the combinatory nature of the problem
    let departure_stops = find_stops_in_time_range(
        hrdf.data_storage(),
        origin_point_latitude,
        origin_point_longitude,
        departure_at,
        time_limit,
    )
    .into_iter()
    .take(5)
    .collect::<Vec<_>>();

    let departure_stop_coord = departure_stops
        .first()
        .unwrap()
        .wgs84_coordinates()
        .unwrap();
    let mut isochrones = Vec::new();

    log::info!(
        "Time for finding the departure_stops : {:.2?}",
        start_time.elapsed()
    );

    let start_time = Instant::now();
    // then go over all these stops to compute each attainable route
    let routes = departure_stops
        .par_iter()
        .map(|departure_stop| {
            // departure_stop_coord = departure_stop.wgs84_coordinates().unwrap();
            // The departure time is calculated according to the time it takes to walk to the departure stop.
            let (adjusted_departure_at, adjusted_time_limit) = adjust_departure_at(
                departure_at,
                time_limit,
                origin_point_latitude,
                origin_point_longitude,
                departure_stop,
            );

            let mut local_routes: Vec<_> = find_reachable_stops_within_time_limit(
                hrdf,
                departure_stop.id(),
                adjusted_departure_at,
                adjusted_time_limit,
                verbose,
            )
            .into_iter()
            .filter(|route| {
                // Keeps only stops in Switzerland.
                let stop_id = route.sections().last().unwrap().arrival_stop_id();
                stop_id.to_string().starts_with("85")
            })
            .collect();

            // A false route is created to represent the point of origin in the results.
            let (easting, northing) = wgs84_to_lv95(origin_point_latitude, origin_point_longitude);
            let route = Route::new(
                NaiveDateTime::default(),
                departure_at,
                vec![RouteSection::new(
                    None,
                    0,
                    Some(Coordinates::default()),
                    Some(Coordinates::default()),
                    0,
                    Some(Coordinates::new(CoordinateSystem::LV95, easting, northing)),
                    Some(Coordinates::default()),
                    Some(NaiveDateTime::default()),
                    Some(NaiveDateTime::default()),
                    Some(0),
                )],
            );
            local_routes.push(route);
            local_routes
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    log::info!("Time for finding the routes : {:.2?}", start_time.elapsed());

    let start_time = Instant::now();

    let data = get_data(routes.into_iter(), departure_at);

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

    for i in 0..isochrone_count {
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

        isochrones.push(Isochrone::new(
            polygons,
            current_time_limit.num_minutes() as u32,
        ));
    }

    log::info!(
        "Time for finding the isochrones : {:.2?}",
        start_time.elapsed()
    );
    IsochroneMap::new(
        isochrones,
        departure_stop_coord,
        convert_bounding_box_to_wgs84(bounding_box),
    )
}

// Find the stop in walking range. The stops are sorted by time to destination
fn find_stops_in_time_range(
    data_storage: &DataStorage,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
) -> Vec<&Stop> {
    let mut stops = data_storage
        .stops()
        .entries()
        .into_iter()
        // Only considers stops in Switzerland.
        .filter(|stop| stop.id().to_string().starts_with("85"))
        .filter(|stop| stop.wgs84_coordinates().is_some())
        .filter(|stop| {
            adjust_departure_at(
                departure_at,
                time_limit,
                origin_point_latitude,
                origin_point_longitude,
                stop,
            )
            .1
            .num_minutes()
                > 0
        })
        // The stop list cannot be empty.
        .collect::<Vec<_>>();
    stops.sort_by(|lhs, rhs| {
        adjust_departure_at(
            departure_at,
            time_limit,
            origin_point_latitude,
            origin_point_longitude,
            rhs,
        )
        .1
        .num_minutes()
        .cmp(
            &adjust_departure_at(
                departure_at,
                time_limit,
                origin_point_latitude,
                origin_point_longitude,
                lhs,
            )
            .1
            .num_minutes(),
        )
    });
    stops
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

fn adjust_departure_at(
    departure_at: NaiveDateTime,
    time_limit: Duration,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_stop: &Stop,
) -> (NaiveDateTime, Duration) {
    let distance = {
        let coord = departure_stop.wgs84_coordinates().unwrap();

        haversine_distance(
            origin_point_latitude,
            origin_point_longitude,
            coord.latitude().expect("Wrong coordinate system"),
            coord.longitude().expect("Wrong coordinate system"),
        ) * 1000.0
    };

    let duration = distance_to_time(distance, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);

    let adjusted_departure_at = departure_at.checked_add_signed(duration).unwrap();
    let adjusted_time_limit = time_limit - duration;

    (adjusted_departure_at, adjusted_time_limit)
}

fn get_data(
    routes: impl Iterator<Item = Route>,
    departure_at: NaiveDateTime,
) -> Vec<(Coordinates, Duration)> {
    routes
        .filter_map(|route| {
            let coord = route
                .sections()
                .last()
                .unwrap()
                .arrival_stop_lv95_coordinates();

            let duration = route.arrival_at() - departure_at;
            coord.zip(Some(duration))
        })
        .collect()
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
        lv95_to_wgs84(bounding_box.0 .0, bounding_box.0 .1),
        lv95_to_wgs84(bounding_box.1 .0, bounding_box.1 .1),
    )
}
