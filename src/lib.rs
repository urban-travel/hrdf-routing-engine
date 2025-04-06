mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

use std::error::Error;

use chrono::Duration;
use geo::Area;
use hrdf_parser::Hrdf;
pub use isochrone::compute_isochrones;
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

pub fn run_test(hrdf: Hrdf) -> Result<(), Box<dyn Error>> {
    let origin_point_latitude = 46.183870262988584;
    let origin_point_longitude = 6.12213134765625;
    let departure_at = create_date_time(2025, 04, 1, 19, 3);
    let time_limit = Duration::minutes(10);
    let isochrone_interval = Duration::minutes(5);
    let display_mode = isochrone::IsochroneDisplayMode::ContourLine;
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

    iso.isochrones().iter().enumerate().for_each(|(num, i)| {
        let mut svg_polygon = SvgPolygon::new();
        let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
        let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);
        for (num_p, p) in i.to_polygons().iter().enumerate() {
            let points = p
                .exterior()
                .coords()
                .map(|coord| format!("{},{}", coord.x, coord.y))
                .collect::<Vec<_>>();

            (min_x, max_x, min_y, max_y) = p.exterior().coords().fold(
                (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
                |(min_x, max_x, min_y, max_y), coord| {
                    (
                        min_x.min(coord.x),
                        max_x.max(coord.x),
                        min_y.min(coord.y),
                        max_y.max(coord.y),
                    )
                },
            );
            svg_polygon.append(
                SvgPolygon::new()
                    .set("fill", "none")
                    .set("stroke", "black")
                    .set("points", points.join(" ")),
            );

            for i in p.interiors().iter() {
                let int_points = i
                    .coords()
                    .map(|coord| format!("{},{}", coord.x, coord.y))
                    .collect::<Vec<_>>()
                    .join(" ");
                svg_polygon.append(
                    SvgPolygon::new()
                        .set("fill", "black")
                        .set("stroke", "black")
                        .set("points", int_points),
                );
            }
        }
        let document = Document::new()
            .set("viewBox", (min_x, min_y, max_x - min_x, max_y - min_y))
            .add(svg_polygon);
        svg::save(format!("polygon_{num}.svg"), &document).unwrap();
    });
    println!(
        "{:?}",
        iso.isochrones()
            .iter()
            .enumerate()
            .map(|(num, i)| i
                .to_polygons()
                .into_iter()
                .enumerate()
                .map(|(num_p, p)| {
                    println!("{num} isochrone, num polygon {num_p} = {:?}", p);
                    p.unsigned_area()
                })
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
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
