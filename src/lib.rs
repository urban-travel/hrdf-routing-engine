mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

use std::error::Error;

use chrono::Duration;
use geo::BoundingRect;
use hrdf_parser::Hrdf;
pub use isochrone::compute_isochrones;
pub use isochrone::IsochroneDisplayMode;
pub use routing::find_reachable_stops_within_time_limit;
pub use routing::plan_journey;
pub use routing::Route;
pub use routing::RouteSection;
use svg::node::element::Polygon as SvgPolygon;
use svg::Document;
use svg::Node;
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

    let polys = iso.get_polygons();

    polys.iter().enumerate().for_each(|(num, p)| {
        let bounding_rect = p.bounding_rect().unwrap();
        let (min_x, min_y) = bounding_rect.min().x_y();
        let (max_x, max_y) = bounding_rect.max().x_y();

        let document = p.iter().fold(
            Document::new().set(
                "viewBox",
                (
                    min_x / 100.0,
                    min_y / 100.0,
                    max_x / 100.0 - min_x / 100.0,
                    max_y / 100.0 - min_y / 100.0,
                ),
            ),
            |mut doc, pi| {
                for int in pi.interiors() {
                    let points_int = int
                        .coords()
                        .map(|coord| {
                            format!(
                                "{},{}",
                                coord.x / 100.0,
                                (min_y + (max_y - coord.y)) / 100.0
                            )
                        })
                        .collect::<Vec<_>>();
                    doc.append(
                        SvgPolygon::new()
                            .set("fill", "black")
                            .set("stroke", "black")
                            .set("points", points_int.join(" ")),
                    );
                }
                let points_ext = pi
                    .exterior()
                    .coords()
                    .map(|coord| {
                        format!(
                            "{},{}",
                            coord.x / 100.0,
                            (min_y + (max_y - coord.y)) / 100.0
                        )
                    })
                    .collect::<Vec<_>>();

                doc.add(
                    SvgPolygon::new()
                        .set("fill", "none")
                        .set("stroke", "red")
                        .set("points", points_ext.join(" ")),
                )
            },
        );
        svg::save(format!("polygon_{display_mode:?}_{num}.svg"), &document).unwrap();
    });
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
