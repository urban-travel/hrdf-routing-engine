use chrono::Duration;
use contour::ContourBuilder;
use geo::{BooleanOps, MapCoordsInPlace, MultiPolygon};
use hrdf_parser::{CoordinateSystem, Coordinates};
use kd_tree::{KdPoint, KdTree};
use orx_parallel::*;

use super::{
    constants::WALKING_SPEED_IN_KILOMETERS_PER_HOUR,
    utils::{distance_between_2_points, distance_to_time, lv95_to_wgs84, time_to_distance},
};

pub fn create_grid(
    data: &[(Coordinates, Duration)],
    bounding_box: ((f64, f64), (f64, f64)),
    time_limit: Duration,
    num_points: usize,
    num_threads: usize,
) -> (Vec<(Coordinates, Duration)>, usize, usize, f64) {
    let dist_x = bounding_box.1.0 - bounding_box.0.0;
    let dist_y = bounding_box.1.1 - bounding_box.0.1;
    let max = dist_x.max(dist_y);
    let dx = max / num_points as f64;

    let num_points_x = ((bounding_box.1.0 - bounding_box.0.0) / dx).ceil() as usize;
    let num_points_y = ((bounding_box.1.1 - bounding_box.0.1) / dx).ceil() as usize;

    let tree = KdTree::build_by_ordered_float(
        data.iter()
            .map(|&(coord, duration)| MyPoint {
                point: [
                    coord.easting().expect("Wrong coordinate system"),
                    coord.northing().expect("Wrong coordinate system"),
                ],
                coord,
                duration,
            })
            .collect(),
    );

    let grid = (0..num_points_y)
        .into_par()
        .num_threads(num_threads)
        .flat_map(|y| {
            let mut result = Vec::new();
            let y = bounding_box.0.1 + dx * y as f64;

            for x in 0..num_points_x {
                let x = bounding_box.0.0 + dx * x as f64;

                let coord = Coordinates::new(CoordinateSystem::LV95, x, y);

                let points = tree.within_radius(
                    &[
                        coord.easting().expect("Wrong coordinate system"),
                        coord.northing().expect("Wrong coordinate system"),
                    ],
                    time_to_distance(time_limit, WALKING_SPEED_IN_KILOMETERS_PER_HOUR),
                );

                if points.is_empty() {
                    result.push((coord, time_limit * 2));
                    continue;
                }

                let duration = points
                    .iter()
                    .map(|point| {
                        let distance = distance_between_2_points(coord, point.coord());

                        point.duration()
                            + distance_to_time(distance, WALKING_SPEED_IN_KILOMETERS_PER_HOUR)
                    })
                    .min()
                    .unwrap();

                result.push((coord, duration));
            }

            result
        })
        .collect::<Vec<(Coordinates, Duration)>>();

    (grid, num_points_x, num_points_y, dx)
}

pub fn get_polygons(
    grid: &[(Coordinates, Duration)],
    num_points_x: usize,
    num_points_y: usize,
    min_point: (f64, f64),
    time_limit: Duration,
    dx: f64,
) -> MultiPolygon {
    let values: Vec<_> = grid
        .iter()
        .map(
            |&(_, duration)| {
                if duration <= time_limit { 1.0 } else { 0.0 }
            },
        )
        .collect();

    let contour_builder = ContourBuilder::new(num_points_x, num_points_y, true);
    let contours = contour_builder.contours(&values, &[0.5]).unwrap();

    contours
        .into_iter()
        .map(|c| {
            let (mut poly, _) = c.into_inner();
            poly.map_coords_in_place(|c| {
                geo::Coord::from(lv95_to_wgs84(
                    min_point.0 + dx * c.x,
                    min_point.1 + dx * c.y,
                ))
            });
            poly
        })
        .fold(MultiPolygon::new(vec![]), |res, p| res.union(&p))
}

#[derive(Debug)]
struct MyPoint {
    point: [f64; 2],
    coord: Coordinates,
    duration: Duration,
}

impl KdPoint for MyPoint {
    type Scalar = f64;
    type Dim = typenum::U2;

    fn at(&self, k: usize) -> f64 {
        self.point[k]
    }
}

impl MyPoint {
    pub fn coord(&self) -> Coordinates {
        self.coord
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }
}
