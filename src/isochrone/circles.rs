use std::f64::consts::PI;

use chrono::Duration;
use geo::{BooleanOps, LineString, MultiPolygon, Polygon};
use hrdf_parser::Coordinates;
use orx_parallel::*;

use super::utils::{lv95, lv95_to_wgs84, walking_distance};

const CIRCLE_POINTS: usize = 6;

/// The union, in WGS84, of the circles that can be walked in the time left around each stop
/// (LV95) reached within `time_limit`.
pub fn get_polygons(
    data: &[(Coordinates, Duration)],
    time_limit: Duration,
    num_threads: usize,
) -> MultiPolygon {
    let mut reachable: Vec<_> = data
        .iter()
        .filter(|(_, duration)| Duration::zero() <= *duration && *duration <= time_limit)
        .collect();
    // Group nearby circles so early unions can simplify overlapping boundaries: sort by
    // easting, then each strip by northing.
    reachable.sort_unstable_by(|(lhs, _), (rhs, _)| lv95(lhs).0.total_cmp(&lv95(rhs).0));
    let strip_size = (reachable.len() as f64).sqrt().ceil().max(1.0) as usize;
    for strip in reachable.chunks_mut(strip_size) {
        strip.sort_unstable_by(|(lhs, _), (rhs, _)| lv95(lhs).1.total_cmp(&lv95(rhs).1));
    }

    let mut polygons = reachable
        .par()
        .chunk_size(50)
        .num_threads(num_threads)
        .map(|(center, duration)| {
            let (e, n) = lv95(center);
            MultiPolygon::new(vec![circle(e, n, walking_distance(time_limit - *duration))])
        })
        .collect::<Vec<_>>();

    // Union pairs level by level: folding one circle at a time into the whole boundary is slow.
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

/// A polygon approximating the circle around an LV95 point, in WGS84.
fn circle(e: f64, n: f64, radius: f64) -> Polygon {
    let step = 2.0 * PI / CIRCLE_POINTS as f64;
    let points = (0..CIRCLE_POINTS).map(|i| {
        let angle = i as f64 * step;
        lv95_to_wgs84(e + radius * angle.cos(), n + radius * angle.sin())
    });
    Polygon::new(LineString::from(points.collect::<Vec<_>>()), vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isochrone::utils::multi_polygon_to_lv95;
    use geo::{Area, Contains, Point, Rect};
    use hrdf_parser::CoordinateSystem;

    /// The circles unioned one by one: the reference for the balanced union.
    fn incremental_polygons(
        data: &[(Coordinates, Duration)],
        time_limit: Duration,
    ) -> MultiPolygon {
        data.iter()
            .filter(|(_, duration)| *duration <= time_limit)
            .map(|point| get_polygons(std::slice::from_ref(point), time_limit, 1))
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
        let holes = |m: &MultiPolygon| m.iter().map(|p| p.interiors().len()).sum::<usize>();
        assert_eq!(holes(actual), holes(expected), "holes");
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
        let reference = incremental_polygons(&data, limit);
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
                let result = get_polygons(&data, limit, threads);
                assert_equivalent(&result, &reference);
                assert_equivalent(&result.difference(&excluded), &clipped_reference);
                let hole = lv95_to_wgs84(2_600_000.0, 1_200_000.0);
                assert!(!result.contains(&Point::new(hole.0, hole.1)));
            }
        }
    }

    #[test]
    fn stops_reached_after_the_limit_are_left_out() {
        let data = [
            point(0.0, 0.0, 9),
            point(10000.0, 0.0, 29),
            point(20000.0, 0.0, 30),
            point(30000.0, 0.0, 31),
        ];
        let limit = Duration::minutes(30);
        let reference = incremental_polygons(&data, limit);
        assert_eq!(reference.0.len(), 2);
        for threads in [1, 4] {
            assert_equivalent(&get_polygons(&data, limit, threads), &reference);
        }
    }

    #[test]
    fn zero_radius_circle_preserves_the_positive_circle() {
        let data = [point(10000.0, 0.0, 30), point(0.0, 0.0, 10)];
        let limit = Duration::minutes(30);
        let result = get_polygons(&data, limit, 1);
        assert!(multi_polygon_to_lv95(&result).unsigned_area() > 0.0);
        assert_equivalent(&result, &incremental_polygons(&data, limit));
    }
}
