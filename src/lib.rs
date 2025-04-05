mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

pub use isochrone::compute_isochrones;
pub use routing::find_reachable_stops_within_time_limit;
pub use routing::plan_journey;
pub use routing::Route;
pub use routing::RouteSection;

pub use debug::run_debug;
pub use service::run_service;

#[cfg(test)]
mod tests {
    use crate::debug::{test_find_reachable_stops_within_time_limit, test_plan_journey};

    use super::*;
    use hrdf_parser::{Hrdf, Version};
    use test_log::test;

    #[test(tokio::test)]
    async fn debug() {
        let hrdf = Hrdf::new(
            Version::V_5_40_41_2_0_7,
            "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
            false,
            None,
        )
        .await
        .unwrap();

        test_plan_journey(&hrdf);
        test_find_reachable_stops_within_time_limit(&hrdf);
    }
}
