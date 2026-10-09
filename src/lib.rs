mod app;
mod cli;
mod debug;
mod error;
mod isochrone;
mod planner;
mod routing;
mod service;

#[cfg(feature = "hectare")]
pub use app::run_surface_per_ha;
pub use app::{Pick, run_comparison, run_isochrones};
#[cfg(feature = "hectare")]
pub use cli::HectareArgs;
pub use cli::{Cli, JourneyArgs, Mode};
pub use debug::run_debug;
pub use error::RResult;
pub use isochrone::externals::lakes;
#[cfg(feature = "hectare")]
pub use isochrone::externals::{HectareRecord, hectare_records};
pub use isochrone::{
    IsochroneArgs, IsochroneDisplayMode, IsochroneMap, compute_average_isochrones,
    compute_isochrones, compute_optimal_isochrones, compute_worst_isochrones,
};
pub use routing::{
    Route, RouteSection, Router, compute_routes_from_origin, compute_routes_to_destination,
    find_origin_stops_within_time_limit, find_reachable_stops_within_time_limit, plan_journey,
    plan_journey_reverse,
};
pub use service::run_service;
