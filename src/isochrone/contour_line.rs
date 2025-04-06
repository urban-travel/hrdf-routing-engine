use chrono::Duration;
use contour::ContourBuilder;
use hrdf_parser::{CoordinateSystem, Coordinates};
use kd_tree::{KdPoint, KdTree};

use super::{
    constants::{GRID_SPACING_IN_METERS, WALKING_SPEED_IN_KILOMETERS_PER_HOUR},
    utils::{distance_between_2_points, distance_to_time, lv95_to_wgs84, time_to_distance},
};

use rayon::prelude::*;

pub fn create_grid(
    data: &[(Coordinates, Duration)],
    bounding_box: ((f64, f64), (f64, f64)),
    time_limit: Duration,
) -> (Vec<(Coordinates, Duration)>, usize, usize) {
    let num_points_x =
        ((bounding_box.1 .0 - bounding_box.0 .0) / GRID_SPACING_IN_METERS).ceil() as usize;
    let num_points_y =
        ((bounding_box.1 .1 - bounding_box.0 .1) / GRID_SPACING_IN_METERS).ceil() as usize;

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
        .into_par_iter()
        .map(|y| {
            let mut result = Vec::new();
            let y = bounding_box.0 .1 + GRID_SPACING_IN_METERS * y as f64;

            for x in 0..num_points_x {
                let x = bounding_box.0 .0 + GRID_SPACING_IN_METERS * x as f64;

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
        .flatten()
        .collect::<Vec<(Coordinates, Duration)>>();

    (grid, num_points_x, num_points_y)
}

pub fn get_polygons(
    grid: &[(Coordinates, Duration)],
    num_points_x: usize,
    num_points_y: usize,
    min_point: (f64, f64),
    time_limit: Duration,
) -> Vec<Vec<Coordinates>> {
    let values: Vec<_> = grid
        .iter()
        .map(
            |&(_, duration)| {
                if duration <= time_limit {
                    1.0
                } else {
                    0.0
                }
            },
        )
        .collect();

    let contour_builder = ContourBuilder::new(num_points_x, num_points_y, true);
    let contours = contour_builder.contours(&values, &[0.5]).unwrap();

    contours[0]
        .geometry()
        .0
        .iter()
        .map(|polygon| {
            polygon
                .exterior()
                .into_iter()
                .map(|coord| {
                    let lv95 = (
                        min_point.0 + GRID_SPACING_IN_METERS * coord.x,
                        min_point.1 + GRID_SPACING_IN_METERS * coord.y,
                    );
                    let wgs84 = lv95_to_wgs84(lv95.0, lv95.1);
                    Coordinates::new(CoordinateSystem::WGS84, wgs84.0, wgs84.1)
                })
                .collect()
        })
        .collect()
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
