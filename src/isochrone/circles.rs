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

/// Returns the polygons in wgs84 coordinates from LV95 coordinates.
// TODO: create two versions of this function for LV95 and WGS84
pub fn get_polygons(
    data: &[(Coordinates, Duration)],
    time_limit: Duration,
    prev_time_limit: Duration,
    num_circle_points: usize,
    num_threads: usize,
) -> MultiPolygon {
    let mut reachable: Vec<_> = data
        .iter()
        .filter(|(_, duration)| prev_time_limit <= *duration && *duration <= time_limit)
        .collect();
    // Group nearby circles so early unions can simplify overlapping boundaries.
    // Sort easting strips by northing, retaining all original coordinates.
    reachable.sort_unstable_by(|(lhs, _), (rhs, _)| {
        lhs.easting()
            .expect("Wrong coordinate system")
            .total_cmp(&rhs.easting().expect("Wrong coordinate system"))
    });
    let strip_size = (reachable.len() as f64).sqrt().ceil().max(1.0) as usize;
    for strip in reachable.chunks_mut(strip_size) {
        strip.sort_unstable_by(|(lhs, _), (rhs, _)| {
            lhs.northing()
                .expect("Wrong coordinate system")
                .total_cmp(&rhs.northing().expect("Wrong coordinate system"))
        });
    }

    let mut polygons = reachable
        .par()
        .chunk_size(50)
        .num_threads(num_threads)
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
            MultiPolygon::new(vec![Polygon::new(LineString::from(polygon), vec![])])
        })
        .collect::<Vec<_>>();

    // Combine similarly sized groups at each level. A sequential fold repeatedly
    // overlays every new circle onto the entire accumulated boundary.
    while polygons.len() > 1 {
        let pairs: Vec<_> = polygons.chunks(2).collect();
        polygons = pairs
            .par()
            .num_threads(num_threads)
            .map(|pair| match pair {
                [left, right] => left.union(right),
                [last] => last.clone(),
                _ => unreachable!(),
            })
            .collect();
    }
    polygons.pop().expect("Could not compute Polygon")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isochrone::utils::multi_polygon_to_lv95;
    use geo::{Area, BooleanOps, Contains, Point, Rect};

    // Original sequential reduction, retained only for geometry regression checks.
    fn incremental_polygons(
        data: &[(Coordinates, Duration)],
        time_limit: Duration,
        prev_time_limit: Duration,
        num_circle_points: usize,
    ) -> MultiPolygon {
        data.iter()
            .filter(|(_, duration)| prev_time_limit <= *duration && *duration <= time_limit)
            .map(|point| {
                // A single input takes the unchanged circle-generation path above.
                get_polygons(
                    std::slice::from_ref(point),
                    time_limit,
                    prev_time_limit,
                    num_circle_points,
                    1,
                )
            })
            .reduce(|lhs, rhs| lhs.union(&rhs))
            .expect("Could not compute Polygon")
    }

    fn point(x: f64, y: f64, minutes: i64) -> (Coordinates, Duration) {
        (
            Coordinates::new(CoordinateSystem::LV95, 2_600_000.0 + x, 1_200_000.0 + y),
            Duration::minutes(minutes),
        )
    }

    fn assert_equivalent(actual: &MultiPolygon, expected: &MultiPolygon) {
        let expected_area = multi_polygon_to_lv95(expected).unsigned_area();
        let difference_area = multi_polygon_to_lv95(&actual.xor(expected)).unsigned_area();
        assert!(
            difference_area <= expected_area * 1e-6 + 0.01,
            "symmetric difference {difference_area} m² for reference area {expected_area} m²"
        );
        assert_eq!(actual.0.len(), expected.0.len(), "disconnected regions");
        assert_eq!(
            actual.iter().map(|p| p.interiors().len()).sum::<usize>(),
            expected.iter().map(|p| p.interiors().len()).sum::<usize>(),
            "holes"
        );
    }

    #[test]
    fn balanced_union_preserves_holes_disconnected_regions_and_clipping() {
        // Overlapping 667 m circles around a 1 km ring leave a central hole.
        let mut data: Vec<_> = (0..12)
            .map(|i| {
                let angle = 2.0 * PI * i as f64 / 12.0;
                point(1000.0 * angle.cos(), 1000.0 * angle.sin(), 20)
            })
            .collect();
        data.extend([point(5000.0, 0.0, 20), point(5100.0, 0.0, 20)]);
        // Duplicate and contained circles, a zero-radius circle, and a late stop.
        data.extend([
            data[0],
            point(5000.0, 0.0, 25),
            point(10000.0, 0.0, 30),
            point(20000.0, 0.0, 31),
        ]);
        let limit = Duration::minutes(30);
        let reference = incremental_polygons(&data, limit, Duration::zero(), 6);
        assert_eq!(reference.0.len(), 2);
        assert_eq!(
            reference.iter().map(|p| p.interiors().len()).sum::<usize>(),
            1
        );

        let (x0, y0) = lv95_to_wgs84(2_604_900.0, 1_199_900.0);
        let (x1, y1) = lv95_to_wgs84(2_605_200.0, 1_200_100.0);
        let excluded = MultiPolygon::new(vec![Rect::new((x0, y0), (x1, y1)).to_polygon()]);
        let clipped_reference = reference.difference(&excluded);
        assert_eq!(
            clipped_reference
                .iter()
                .map(|p| p.interiors().len())
                .sum::<usize>(),
            2
        );

        for threads in [1, 4] {
            for reverse in [false, true] {
                if reverse {
                    data.reverse();
                }
                let result = get_polygons(&data, limit, Duration::zero(), 6, threads);
                assert_equivalent(&result, &reference);
                assert_equivalent(&result.difference(&excluded), &clipped_reference);
                let hole = lv95_to_wgs84(2_600_000.0, 1_200_000.0);
                assert!(!result.contains(&Point::new(hole.0, hole.1)));
            }
        }
    }

    #[test]
    fn balanced_union_preserves_time_filter_boundaries() {
        let data = [
            point(0.0, 0.0, 9),
            point(10000.0, 0.0, 10),
            point(20000.0, 0.0, 29),
            point(30000.0, 0.0, 30),
            point(40000.0, 0.0, 31),
        ];
        let limit = Duration::minutes(30);
        let previous = Duration::minutes(10);
        let reference = incremental_polygons(&data, limit, previous, 6);
        assert_eq!(reference.0.len(), 2);
        for threads in [1, 4] {
            assert_equivalent(
                &get_polygons(&data, limit, previous, 6, threads),
                &reference,
            );
        }
    }

    #[test]
    fn zero_radius_circle_preserves_the_positive_circle() {
        let data = [point(10000.0, 0.0, 30), point(0.0, 0.0, 10)];
        let limit = Duration::minutes(30);
        let reference = incremental_polygons(&data, limit, Duration::zero(), 6);
        let result = get_polygons(&data, limit, Duration::zero(), 6, 1);
        assert!(multi_polygon_to_lv95(&result).unsigned_area() > 0.0);
        assert_equivalent(&result, &reference);
    }
}
