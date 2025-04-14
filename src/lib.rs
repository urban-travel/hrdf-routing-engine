mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

use std::error::Error;

use chrono::Duration;
use hrdf_parser::Coordinates;
use hrdf_parser::Hrdf;
pub use isochrone::IsochroneDisplayMode;
pub use isochrone::compute_isochrones;
use isochrone::compute_optimal_isochrones;
use isochrone::wgs84_to_lv95;
pub use routing::Route;
pub use routing::RouteSection;
pub use routing::find_reachable_stops_within_time_limit;
pub use routing::plan_journey;
use utils::create_date_time;

pub use debug::run_debug;
pub use service::run_service;

pub fn run_test(hrdf: Hrdf, display_mode: IsochroneDisplayMode) -> Result<(), Box<dyn Error>> {
    let origin_point_latitude = 46.20956654;
    let origin_point_longitude = 6.13536000;

    let departure_at = create_date_time(2025, 4, 10, 15, 36);
    let time_limit = Duration::minutes(60);
    let isochrone_interval = Duration::minutes(10);
    let verbose = true;
    let (x, y) = wgs84_to_lv95(origin_point_latitude, origin_point_longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);

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
    iso.write_svg(
        &format!(
            "isochrones_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;

    let opt_iso = compute_optimal_isochrones(
        &hrdf,
        origin_point_latitude,
        origin_point_longitude,
        departure_at,
        time_limit,
        isochrone_interval,
        Duration::minutes(30),
        display_mode,
        false,
    );

    #[cfg(feature = "svg")]
    opt_iso.write_svg(
        &format!(
            "optimal_isochrones_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;

    Ok(())
}

pub fn run_comparison(
    hrdf_2024: Hrdf,
    hrdf_2025: Hrdf,
    display_mode: IsochroneDisplayMode,
) -> Result<(), Box<dyn Error>> {
    let origin_point_latitude = 46.183870262988584;
    let origin_point_longitude = 6.12213134765625;
    let departure_at_2024 = create_date_time(2024, 4, 1, 12, 0);
    let departure_at_2025 = create_date_time(2025, 4, 1, 12, 0);
    let time_limit = Duration::minutes(60);
    let isochrone_interval = Duration::minutes(60);
    let verbose = false;
    let (x, y) = wgs84_to_lv95(origin_point_latitude, origin_point_longitude);
    let coord = Coordinates::new(hrdf_parser::CoordinateSystem::LV95, x, y);
    let opt_duration = Duration::minutes(720);

    let isochrones_2024 = compute_optimal_isochrones(
        &hrdf_2024,
        origin_point_latitude,
        origin_point_longitude,
        departure_at_2024,
        time_limit,
        isochrone_interval,
        opt_duration,
        display_mode,
        verbose,
    );
    #[cfg(feature = "svg")]
    isochrones_2024.write_svg(
        &format!(
            "isochrones_2024_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;
    println!(
        "time = {}, surface = {}, max_distance = {}",
        isochrones_2024.departure_at(),
        isochrones_2024.compute_last_area(),
        isochrones_2024.compute_max_distance(coord).1
    );

    let isochrones_2025 = compute_optimal_isochrones(
        &hrdf_2025,
        origin_point_latitude,
        origin_point_longitude,
        departure_at_2025,
        time_limit,
        isochrone_interval,
        opt_duration,
        display_mode,
        verbose,
    );
    #[cfg(feature = "svg")]
    isochrones_2025.write_svg(
        &format!(
            "isochrones_2025_{}_{}.svg",
            time_limit.num_minutes(),
            isochrone_interval.num_minutes()
        ),
        1.0 / 100.0,
        Some(coord),
    )?;
    println!(
        "time = {}, surface = {}, max_distance = {}",
        isochrones_2025.departure_at(),
        isochrones_2025.compute_last_area(),
        isochrones_2025.compute_max_distance(coord).1
    );

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
