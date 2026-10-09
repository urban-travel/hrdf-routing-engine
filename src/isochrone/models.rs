use chrono::NaiveDateTime;
use geo::{Area, Contains, MultiPolygon};
use hrdf_parser::Coordinates;
use serde::{Deserialize, Serialize};

use super::utils::{multi_polygon_to_lv95, wgs84_to_lv95};

#[derive(Debug, Serialize, Default)]
pub struct IsochroneMap {
    isochrones: Vec<Isochrone>,
    areas: Vec<f64>,
    max_distances: Vec<((f64, f64), f64)>,
    departure_stop_coord: Coordinates,
    departure_at: NaiveDateTime,
    bounding_box: ((f64, f64), (f64, f64)),
}

impl IsochroneMap {
    pub fn new(
        isochrones: Vec<Isochrone>,
        areas: Vec<f64>,
        max_distances: Vec<((f64, f64), f64)>,
        departure_stop_coord: Coordinates,
        departure_at: NaiveDateTime,
        bounding_box: ((f64, f64), (f64, f64)),
    ) -> Self {
        Self {
            isochrones,
            areas,
            max_distances,
            departure_stop_coord,
            departure_at,
            bounding_box,
        }
    }

    pub fn compute_areas(&self) -> Vec<f64> {
        self.isochrones.iter().map(|i| i.compute_area()).collect()
    }

    pub fn compute_max_distances(&self, c: Coordinates) -> Vec<((f64, f64), f64)> {
        self.isochrones
            .iter()
            .map(|i| i.compute_max_distance(c))
            .collect()
    }

    /// The farthest point of all isochrones from `c`, and its distance.
    pub fn compute_max_distance(&self, c: Coordinates) -> ((f64, f64), f64) {
        let farthest =
            |best: ((f64, f64), f64), d: ((f64, f64), f64)| if d.1 > best.1 { d } else { best };
        self.compute_max_distances(c)
            .into_iter()
            .fold(((f64::MIN, f64::MIN), f64::MIN), farthest)
    }

    /// The area of the largest isochrone.
    pub fn compute_max_area(&self) -> f64 {
        self.compute_areas()
            .into_iter()
            .fold(f64::MIN, |max, area| if area > max { area } else { max })
    }

    /// The isochrones, each with holes where the previous one is.
    pub fn get_polygons(&self) -> Vec<MultiPolygon> {
        let original: Vec<_> = self.isochrones.iter().map(|i| i.polygons.clone()).collect();
        let mut polygons = original.clone();
        for i in 0..polygons.len() - 1 {
            for outer in &mut polygons[i + 1] {
                for inner in &original[i] {
                    if outer.contains(inner) {
                        outer.interiors_push(inner.exterior().clone());
                    }
                }
            }
        }
        polygons
    }

    pub fn departure_at(&self) -> NaiveDateTime {
        self.departure_at
    }

    /// Draws the isochrones (and the lines to their farthest point from `c`) in LV95.
    #[cfg(feature = "svg")]
    pub fn write_svg(
        &self,
        path: &str,
        scale_factor: f64,
        c: Option<Coordinates>,
    ) -> crate::RResult<()> {
        use geo::BoundingRect;
        use svg::{
            Document,
            node::element::{Line, Polygon},
        };

        const PALETTE: [(u8, u8, u8); 6] = [
            (0x36, 0xAB, 0x68), // Nearest.
            (0x91, 0xCF, 0x60),
            (0xD7, 0xFF, 0x67),
            (0xFF, 0xD7, 0x67),
            (0xFC, 0x8D, 0x59),
            (0xE2, 0x45, 0x3C), // Furthest.
        ];

        let polys: Vec<_> = self
            .get_polygons()
            .iter()
            .map(multi_polygon_to_lv95)
            .collect();
        let bounding_rect = (polys
            .last()
            .expect("MultiPolygons Vec is empty")
            .bounding_rect())
        .ok_or(crate::error::RError::NoBoundingRect)?;
        let (min_x, min_y) = bounding_rect.min().x_y();
        let (max_x, max_y) = bounding_rect.max().x_y();
        let x = |x: f64| x * scale_factor;
        let y = |y: f64| (min_y + (max_y - y)) * scale_factor;

        // With more isochrones than colors, interpolate between the palette colors.
        let num_polys = polys.len();
        let color_of = |num: usize| {
            let (r, g, b) = if num_polys <= PALETTE.len() {
                PALETTE[num]
            } else {
                let t = num as f64 / (num_polys - 1) as f64 * (PALETTE.len() - 1) as f64;
                let i = (t.floor() as usize).min(PALETTE.len() - 2);
                let frac = t - i as f64;
                let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * frac).round() as u8;
                let (c0, c1) = (PALETTE[i], PALETTE[i + 1]);
                (lerp(c0.0, c1.0), lerp(c0.1, c1.1), lerp(c0.2, c1.2))
            };
            format!("#{r:02X}{g:02X}{b:02X}")
        };

        let view_box = (
            x(min_x),
            min_y * scale_factor,
            (max_x - min_x) * scale_factor,
            (max_y - min_y) * scale_factor,
        );
        let mut document = Document::new().set("viewBox", view_box);
        for (num, polygons) in polys.iter().rev().enumerate() {
            for p in polygons {
                let points: Vec<_> = p
                    .exterior()
                    .coords()
                    .map(|c| format!("{},{}", x(c.x), y(c.y)))
                    .collect();
                let polygon = Polygon::new()
                    .set("fill", color_of(num))
                    .set("stroke", "black");
                document = document.add(polygon.set("points", points.join(" ")));
            }
        }
        if let Some(c) = c {
            for ((px, py), _) in self.compute_max_distances(c).into_iter().rev() {
                let line = Line::new().set("x1", x(px)).set("y1", y(py));
                let line = line
                    .set("x2", x(c.easting().unwrap()))
                    .set("y2", y(c.northing().unwrap()));
                document = document.add(line.set("stroke", "black"));
            }
        }
        svg::save(path, &document)?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Isochrone {
    /// In WGS84.
    polygons: MultiPolygon,
    /// In minutes.
    time_limit: u32,
}

impl Isochrone {
    pub fn new(polygons: MultiPolygon, time_limit: u32) -> Self {
        Self {
            polygons,
            time_limit,
        }
    }

    pub fn polygons(&self) -> &MultiPolygon {
        &self.polygons
    }

    /// In square meters.
    pub fn compute_area(&self) -> f64 {
        multi_polygon_to_lv95(&self.polygons)
            .iter()
            .map(|p| p.unsigned_area())
            .sum()
    }

    /// The point of the isochrone farthest from `c`, in LV95, and its distance in meters.
    pub fn compute_max_distance(&self, c: Coordinates) -> ((f64, f64), f64) {
        let (c_x, c_y) = match (c.easting(), c.northing()) {
            (Some(x), Some(y)) => (x, y),
            _ => wgs84_to_lv95(c.latitude().unwrap(), c.longitude().unwrap()),
        };
        let points = self
            .polygons
            .iter()
            .flat_map(|p| p.exterior())
            .map(|coord| wgs84_to_lv95(coord.x, coord.y));
        points.fold(((f64::MIN, f64::MIN), f64::MIN), |farthest, (x, y)| {
            let distance = f64::sqrt(f64::powi(c_x - x, 2) + f64::powi(c_y - y, 2));
            if distance > farthest.1 {
                ((x, y), distance)
            } else {
                farthest
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMode {
    Circles,
    #[value(name = "contour_line")]
    ContourLine,
}
