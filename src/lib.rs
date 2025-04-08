mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

use std::error::Error;

use chrono::Duration;
use hrdf_parser::Hrdf;
pub use isochrone::compute_isochrones;
pub use isochrone::IsochroneDisplayMode;
pub use routing::find_reachable_stops_within_time_limit;
pub use routing::plan_journey;
pub use routing::Route;
pub use routing::RouteSection;
use utils::create_date_time;

pub use debug::run_debug;
pub use service::run_service;

pub fn run_test(hrdf: Hrdf, display_mode: IsochroneDisplayMode) -> Result<(), Box<dyn Error>> {
    let origin_point_latitude = 46.183870262988584;
    let origin_point_longitude = 6.12213134765625;
    let departure_at = create_date_time(2025, 4, 1, 8, 3);
    let time_limit = Duration::minutes(480);
    let isochrone_interval = Duration::minutes(80);
    let verbose = true;

    #[cfg(feature = "svg")]
    let iso = compute_isochrones(
        &hrdf,
        origin_point_latitude,
        origin_point_longitude,
        departure_at,
        time_limit,
        isochrone_interval,
        display_mode,
        verbose,
    );
    #[cfg(not(feature = "svg"))]
    let _iso = compute_isochrones(
        &hrdf,
        origin_point_latitude,
        origin_point_longitude,
        departure_at,
        time_limit,
        isochrone_interval,
        display_mode,
        verbose,
    );

    #[cfg(feature = "svg")]
    iso.write_svg(&format!(
        "isocrhones_{}_{}.svg",
        time_limit.num_minutes(),
        isochrone_interval.num_minutes()
    ))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::debug::{test_find_reachable_stops_within_time_limit, test_plan_journey};

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
