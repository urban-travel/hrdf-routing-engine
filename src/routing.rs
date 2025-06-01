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

use chrono::NaiveDateTime;
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
