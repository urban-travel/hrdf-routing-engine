mod connections;
mod core;
mod display;
mod exploration;
mod models;
mod route_impl;
mod utils;

use crate::isochrone::utils::adjust_arrival_at;
use crate::isochrone::utils::adjust_departure_at;
use crate::isochrone::utils::wgs84_to_lv95;
use crate::routing::models::Transport;
use hrdf_parser::DataStorage;
use hrdf_parser::Hrdf;
use hrdf_parser::Model;
use hrdf_parser::Stop;
use hrdf_parser::{CoordinateSystem, Coordinates};
pub use models::RouteResult as Route;
pub use models::RouteSectionResult as RouteSection;
use orx_parallel::*;

use connections::DepartureCache;
use core::{compute_routing, compute_routing_reverse, compute_routing_with_cache};

use chrono::{Duration, NaiveDateTime};
use models::RoutingAlgorithmArgs;

/// Finds the fastest route from the departure stop to the arrival stop.
/// It may not be the one which takes the less time since there is no departure
/// time optimization. The only guaratnee is that it si the route that arrives the earliest.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Option<Route> {
    let result = compute_routing(
        hrdf.data_storage(),
        departure_stop_id,
        departure_at,
        max_num_explorable_connections,
        verbose,
        RoutingAlgorithmArgs::solve_from_departure_stop_to_arrival_stop(arrival_stop_id),
    )
    .remove(&arrival_stop_id);

    if verbose && let Some(rou) = &result {
        println!();
        rou.print(hrdf.data_storage());
    }

    result
}

/// Finds the latest possible departure from the departure stop to arrive at the arrival stop by arrival_at.
pub fn plan_journey_reverse(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    arrival_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Option<Route> {
    let result = compute_routing_reverse(
        hrdf.data_storage(),
        arrival_stop_id,
        arrival_at,
        max_num_explorable_connections,
        verbose,
        RoutingAlgorithmArgs::solve_from_departure_stop_to_arrival_stop(departure_stop_id),
    )
    .remove(&departure_stop_id);

    if verbose && let Some(rou) = &result {
        println!();
        rou.print(hrdf.data_storage());
    }

    result
}

/// Finds the route that takes the least time while arriving the earliest possioble.
/// It basically moves from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_shortest_journey_with_reverse(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Option<Route> {
    let route = plan_journey(
        hrdf,
        departure_stop_id,
        arrival_stop_id,
        departure_at,
        max_num_explorable_connections,
        false,
    )?;
    let arrival_at = route.arrival_at();
    println!("=======================================================");
    println!(
        "Dep: {departure_at:?}, Arr: {arrival_at:?}, dep_id: {departure_stop_id}, arr_id: {arrival_stop_id}"
    );

    let route = plan_journey_reverse(
        hrdf,
        departure_stop_id,
        arrival_stop_id,
        arrival_at,
        max_num_explorable_connections,
        false,
    )?;
    let departure_at = route.departure_at();
    let arrival_at = route.arrival_at();
    println!(
        "Dep: {departure_at:?}, Arr: {arrival_at:?}, dep_id: {departure_stop_id}, arr_id: {arrival_stop_id}"
    );
    if verbose {
        println!();
        route.print(hrdf.data_storage());
    }
    Some(route)
}

/// Finds the route that takes the least time while arriving the earliest possioble.
/// It basically moves from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_shortest_journey(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Option<Route> {
    let mut route = plan_journey(
        hrdf,
        departure_stop_id,
        arrival_stop_id,
        departure_at,
        max_num_explorable_connections,
        false,
    )?;
    let mut dep_time = route.departure_at();
    let arrival_at = route.arrival_at();
    dep_time += Duration::minutes(1);
    let route = loop {
        let current_route = plan_journey(
            hrdf,
            departure_stop_id,
            arrival_stop_id,
            dep_time,
            max_num_explorable_connections,
            false,
        );

        if let Some(cr) = current_route {
            if arrival_at < cr.arrival_at() || dep_time >= cr.departure_at() + Duration::minutes(1)
            {
                break route;
            }
            dep_time = cr.departure_at() + Duration::minutes(1);
            route = cr;
        } else {
            break route;
        }
    };
    if verbose {
        println!();
        route.print(hrdf.data_storage());
    }
    Some(route)
}

/// Finds all stops that can be reached within a time limit from the departured stop.
/// The departure date and time must be within the timetable period.
#[allow(dead_code)]
pub fn find_reachable_stops_within_time_limit(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let routes = compute_routing(
        hrdf.data_storage(),
        departure_stop_id,
        departure_at,
        max_num_explorable_connections,
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
        .filter(|stop| stop.wgs84_coordinates().is_some())
        .filter_map(|stop| {
            let remaining_minutes = adjust_departure_at(
                departure_at,
                time_limit,
                origin_point_latitude,
                origin_point_longitude,
                stop,
            )
            .1
            .num_minutes();
            (remaining_minutes > 0).then_some((stop, remaining_minutes))
        })
        // The stop list cannot be empty.
        .collect::<Vec<_>>();
    // Sort by remaining time descending (most slack first), matching the previous behavior.
    stops.sort_by(|(_, lhs), (_, rhs)| rhs.cmp(lhs));
    stops.into_iter().map(|(stop, _)| stop).collect::<Vec<_>>()
}

/// Given a starting point (long/lat) find the Routes given a time limit.
/// We first find num_starting_points stops that are reachable by foot
#[allow(clippy::too_many_arguments)]
pub fn compute_routes_from_origin(
    hrdf: &Hrdf,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    num_starting_points: usize,
    num_threads: usize,
    max_num_explorable_connections: i32,
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

    let departure_cache = DepartureCache::default();
    let mut routes = departure_stops
        .par()
        .num_threads(num_threads)
        .flat_map(|departure_stop| {
            // The departure time is calculated according to the time it takes to walk to the departure stop.
            let (adjusted_departure_at, adjusted_time_limit) = adjust_departure_at(
                departure_at,
                time_limit,
                origin_point_latitude,
                origin_point_longitude,
                departure_stop,
            );
            if verbose {
                log::info!(
                    "Departure stop : {:?}, Adjusted departure at : {:?}, Adjusted time limit : {:?}",
                    departure_stop,
                    adjusted_departure_at,
                    adjusted_time_limit
                );
            }

            let local_routes: Vec<_> = compute_routing_with_cache(
                hrdf.data_storage(),
                departure_stop.id(),
                adjusted_departure_at,
                max_num_explorable_connections,
                verbose,
                RoutingAlgorithmArgs::solve_from_departure_stop_to_reachable_arrival_stops(
                    adjusted_departure_at.checked_add_signed(adjusted_time_limit).unwrap(),
                ),
                &departure_cache,
            ).into_values().collect();

            local_routes
        })
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
            Transport::Unknown,
        )],
    );
    routes.push(route);
    routes
}

/// Finds all origin stops from which the arrival stop can be reached within a time limit.
/// The arrival date and time must be within the timetable period.
#[allow(dead_code)]
pub fn find_origin_stops_within_time_limit(
    hrdf: &Hrdf,
    arrival_stop_id: i32,
    arrival_at: NaiveDateTime,
    time_limit: Duration,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let routes = compute_routing_reverse(
        hrdf.data_storage(),
        arrival_stop_id,
        arrival_at,
        max_num_explorable_connections,
        verbose,
        RoutingAlgorithmArgs::solve_from_arrival_stop_to_reachable_departure_stops(
            arrival_at.checked_sub_signed(time_limit).unwrap(),
        ),
    );
    routes.into_values().collect()
}

// Find stops in walking range of the destination. Sorted by remaining time (descending).
fn find_stops_in_time_range_reverse(
    data_storage: &DataStorage,
    destination_latitude: f64,
    destination_longitude: f64,
    arrival_at: NaiveDateTime,
    time_limit: Duration,
) -> Vec<&Stop> {
    let mut stops = data_storage
        .stops()
        .entries()
        .into_iter()
        .filter(|stop| stop.wgs84_coordinates().is_some())
        .filter(|stop| {
            adjust_arrival_at(
                arrival_at,
                time_limit,
                destination_latitude,
                destination_longitude,
                stop,
            )
            .1
            .num_minutes()
                > 0
        })
        .collect::<Vec<_>>();
    stops.sort_by(|lhs, rhs| {
        adjust_arrival_at(
            arrival_at,
            time_limit,
            destination_latitude,
            destination_longitude,
            rhs,
        )
        .1
        .num_minutes()
        .cmp(
            &adjust_arrival_at(
                arrival_at,
                time_limit,
                destination_latitude,
                destination_longitude,
                lhs,
            )
            .1
            .num_minutes(),
        )
    });
    stops
}

/// Given a destination point (lat/lon) and arrival time, find all origin stops
/// from which the destination can be reached within the time limit.
#[allow(clippy::too_many_arguments)]
pub fn compute_routes_to_destination(
    hrdf: &Hrdf,
    destination_latitude: f64,
    destination_longitude: f64,
    arrival_at: NaiveDateTime,
    time_limit: Duration,
    num_starting_points: usize,
    num_threads: usize,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let arrival_stops = find_stops_in_time_range_reverse(
        hrdf.data_storage(),
        destination_latitude,
        destination_longitude,
        arrival_at,
        time_limit,
    )
    .into_iter()
    .take(num_starting_points)
    .collect::<Vec<_>>();

    let mut routes = arrival_stops
        .par()
        .num_threads(num_threads)
        .flat_map(|arrival_stop| {
            let (adjusted_arrival_at, adjusted_time_limit) = adjust_arrival_at(
                arrival_at,
                time_limit,
                destination_latitude,
                destination_longitude,
                arrival_stop,
            );
            if verbose {
                log::info!(
                    "Arrival stop : {:?}, Adjusted arrival at : {:?}, Adjusted time limit : {:?}",
                    arrival_stop,
                    adjusted_arrival_at,
                    adjusted_time_limit
                );
            }

            let local_routes: Vec<_> = find_origin_stops_within_time_limit(
                hrdf,
                arrival_stop.id(),
                adjusted_arrival_at,
                adjusted_time_limit,
                max_num_explorable_connections,
                verbose,
            );

            local_routes
        })
        .collect::<Vec<_>>();

    // A synthetic route representing the destination point itself.
    let (easting, northing) = wgs84_to_lv95(destination_latitude, destination_longitude);
    let route = Route::new(
        arrival_at,
        NaiveDateTime::default(),
        vec![RouteSection::new(
            None,
            0,
            Some(Coordinates::new(CoordinateSystem::LV95, easting, northing)),
            Some(Coordinates::default()),
            0,
            Some(Coordinates::default()),
            Some(Coordinates::default()),
            Some(NaiveDateTime::default()),
            Some(NaiveDateTime::default()),
            Some(0),
            Transport::Unknown,
        )],
    );
    routes.push(route);
    routes
}
