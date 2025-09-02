use orx_parallel::*;
use std::f64::consts::PI;

use chrono::Duration;
use geo::MultiPolygon;
use geo::{BooleanOps, LineString, Polygon};
use hrdf_parser::{CoordinateSystem, Coordinates};

use super::{
    constants::WALKING_SPEED_IN_KILOMETERS_PER_HOUR,
    utils::{lv95_to_wgs84, time_to_distance},
};

pub fn get_polygons(
    data: &[(Coordinates, Duration)],
    time_limit: Duration,
    num_circle_points: usize,
    num_threads: usize,
) -> MultiPolygon {
    data.par()
        .num_threads(num_threads)
        .filter(|(_, duration)| *duration <= time_limit)
        .map(|(center_lv95, duration)| {
            let distance =
                time_to_distance(time_limit - *duration, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);

            let polygon = generate_lv95_circle_points(
                center_lv95.easting().expect("Wrong coordinate system"),
                center_lv95.northing().expect("Wrong coordinate system"),
                distance,
                num_circle_points,
            )
            .into_iter()
            .map(|lv95| {
                let wgs84 = lv95_to_wgs84(
                    lv95.easting().expect("Wrong coordinate system"),
                    lv95.northing().expect("Wrong coordinate system"),
                );
                (wgs84.0, wgs84.1)
            })
            .collect::<Vec<_>>();
            // Polygon::new(LineString::from(polygon), vec![])
            MultiPolygon::new(vec![Polygon::new(LineString::from(polygon), vec![])])
        })
        .reduce(|lhs, rhs| lhs.union(&rhs))
        .expect("Could not compute Polygon")
}

fn generate_lv95_circle_points(e: f64, n: f64, radius: f64, num_points: usize) -> Vec<Coordinates> {
    let mut points = Vec::new();
    let angle_step = 2.0 * PI / num_points as f64;

    for i in 0..num_points {
        let angle = i as f64 * angle_step;
        let de = radius * angle.cos();
        let dn = radius * angle.sin();
        points.push(Coordinates::new(CoordinateSystem::LV95, e + de, n + dn));
    }

    points
}
