mod connections;
mod constants;
mod core;
mod display;
mod exploration;
mod models;
mod route_impl;
mod utils;

use hrdf_parser::Hrdf;
pub use models::RouteResult as Route;
pub use models::RouteSectionResult as RouteSection;

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
