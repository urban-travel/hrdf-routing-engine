use chrono::Duration;
use contour::ContourBuilder;
use geo::{BooleanOps, MapCoordsInPlace, MultiPolygon};
use hrdf_parser::Coordinates;
use kd_tree::KdTree;
use orx_parallel::*;

use super::utils::{lv95, lv95_to_wgs84, walking_distance, walking_time};

/// Grid step in meters.
const DX: f64 = 100.0;

/// The time to reach each point of a regular LV95 grid: from the nearest reached stop (in
/// walking range), plus the walk. Rows go south to north.
pub struct Grid {
    origin: (f64, f64),
    nx: usize,
    ny: usize,
    durations: Vec<Duration>,
}

impl Grid {
    /// Points out of walking range of every stop get twice the time limit.
    pub fn new(
        data: &[(Coordinates, Duration)],
        bounding_box: ((f64, f64), (f64, f64)),
        time_limit: Duration,
        num_threads: usize,
    ) -> Self {
        let ((x0, y0), (x1, y1)) = bounding_box;
        let (nx, ny) = (
            ((x1 - x0) / DX).ceil() as usize,
            ((y1 - y0) / DX).ceil() as usize,
        );
        let points = data
            .iter()
            .map(|(coord, duration)| (<[f64; 2]>::from(lv95(coord)), *duration));
        let tree = KdTree::build_by_ordered_float(points.collect());
        let durations = (0..ny)
            .into_par()
            .num_threads(num_threads)
            .flat_map(|j| {
                let y = y0 + DX * j as f64;
                let tree = &tree;
                (0..nx)
                    .map(move |i| {
                        let x = x0 + DX * i as f64;
                        let near = tree.within_radius(&[x, y], walking_distance(time_limit));
                        let walk = |[e, n]: [f64; 2]| {
                            walking_time(((e - x).powi(2) + (n - y).powi(2)).sqrt())
                        };
                        near.iter()
                            .map(|(point, duration)| *duration + walk(*point))
                            .min()
                            .unwrap_or(time_limit * 2)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Self {
            origin: (x0, y0),
            nx,
            ny,
            durations,
        }
    }

    /// The point by point average of grids over the same bounding box.
    pub fn average(mut grids: Vec<Grid>) -> Self {
        let count = grids.len() as i32;
        let mut total = grids.pop().expect("Grids was empty");
        for grid in grids {
            total
                .durations
                .iter_mut()
                .zip(grid.durations)
                .for_each(|(t, d)| *t += d);
        }
        total.durations.iter_mut().for_each(|d| *d = *d / count);
        total
    }

    /// The area reached within `time_limit`, in WGS84.
    pub fn polygons(&self, time_limit: Duration) -> MultiPolygon {
        let values: Vec<_> = (self.durations.iter())
            .map(|&d| if d <= time_limit { 1.0 } else { 0.0 })
            .collect();
        let contours = ContourBuilder::new(self.nx, self.ny, true)
            .contours(&values, &[0.5])
            .unwrap();
        let (x0, y0) = self.origin;
        contours
            .into_iter()
            .map(|c| {
                let (mut polygon, _) = c.into_inner();
                polygon.map_coords_in_place(|c| lv95_to_wgs84(x0 + DX * c.x, y0 + DX * c.y).into());
                polygon
            })
            .fold(MultiPolygon::new(vec![]), |res, p| res.union(&p))
    }
}
