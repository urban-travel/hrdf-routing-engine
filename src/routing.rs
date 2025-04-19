mod connections;
mod constants;
mod core;
mod display;
mod exploration;
mod models;
mod route_impl;
mod utils;

use crate::isochrone::utils::adjust_departure_at;
use crate::isochrone::utils::wgs84_to_lv95;
use hrdf_parser::DataStorage;
use hrdf_parser::Hrdf;
use hrdf_parser::Model;
use hrdf_parser::Stop;
use hrdf_parser::{CoordinateSystem, Coordinates};
pub use models::RouteResult as Route;
pub use models::RouteSectionResult as RouteSection;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;

use core::compute_routing;

use chrono::{Duration, NaiveDateTime};
use models::RoutingAlgorithmArgs;

/// Finds the fastest route from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    verbose: bool,
) -> Option<Route> {
    let result = compute_routing(
        hrdf.data_storage(),
        departure_stop_id,
        departure_at,
        verbose,
        RoutingAlgorithmArgs::solve_from_departure_stop_to_arrival_stop(arrival_stop_id),
    )
    .remove(&arrival_stop_id);

    if verbose {
        if let Some(rou) = &result {
            println!();
            rou.print(hrdf.data_storage());
        }
    }

    result
}

/// Finds all stops that can be reached within a time limit from the departured stop.
/// The departure date and time must be within the timetable period.
#[allow(dead_code)]
pub fn find_reachable_stops_within_time_limit(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    verbose: bool,
) -> Vec<Route> {
    let routes = compute_routing(
        hrdf.data_storage(),
        departure_stop_id,
        departure_at,
        verbose,
        RoutingAlgorithmArgs::solve_from_departure_stop_to_reachable_arrival_stops(
            departure_at.checked_add_signed(time_limit).unwrap(),
        ),
    );
    routes.into_values().collect()
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

/// Given a starting point (long/lat) find the Routes given a time limit.
/// We first find num_starting_points stops that are reachable by foot
///
pub fn compute_routes_from_origin(
    hrdf: &Hrdf,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    num_starting_points: usize,
    verbose: bool,
) -> Vec<Route> {
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
    .take(num_starting_points)
    .collect::<Vec<_>>();

    // then go over all these stops to compute each attainable route
    let mut routes = departure_stops
        .par_iter()
        .map(|departure_stop| {
            // The departure time is calculated according to the time it takes to walk to the departure stop.
            let (adjusted_departure_at, adjusted_time_limit) = adjust_departure_at(
                departure_at,
                time_limit,
                origin_point_latitude,
                origin_point_longitude,
                departure_stop,
            );

            let local_routes: Vec<_> = find_reachable_stops_within_time_limit(
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

            local_routes
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

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
    routes.push(route);
    routes
}
