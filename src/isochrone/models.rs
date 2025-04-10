use chrono::NaiveDateTime;
use geo::{Area, Contains, LineString, MultiPolygon, Polygon};
use hrdf_parser::Coordinates;
use serde::Serialize;
use strum_macros::EnumString;

#[cfg(feature = "svg")]
use geo::BoundingRect;
#[cfg(feature = "svg")]
use std::error::Error;
#[cfg(feature = "svg")]
use svg::Document;
#[cfg(feature = "svg")]
use svg::node::element::Polygon as SvgPolygon;

use super::utils::wgs84_to_lv95;

#[derive(Debug, Serialize)]
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

    pub fn compute_max_distance(&self, c: Coordinates) -> ((f64, f64), f64) {
        self.compute_max_distances(c).into_iter().fold(
            ((f64::MIN, f64::MIN), f64::MIN),
            |((m_x, m_y), max), ((x, y), v)| {
                if v > max {
                    ((x, y), v)
                } else {
                    ((m_x, m_y), max)
                }
            },
        )
    }

    pub fn compute_max_area(&self) -> f64 {
        self.compute_areas()
            .into_iter()
            .fold(f64::MIN, |max, v| if v > max { v } else { max })
    }

    pub fn get_polygons(&self) -> Vec<MultiPolygon> {
        let mut polygons = self
            .isochrones
            .iter()
            .map(|i| i.to_polygons())
            .collect::<Vec<_>>();

        let polygons_original = polygons.clone();

        for i in 0..polygons.len() - 1 {
            for p_ext in &mut polygons[i + 1] {
                for p_int in &polygons_original[i] {
                    if p_ext.contains(p_int) {
                        p_ext.interiors_push(p_int.exterior().clone());
                    }
                }
            }
        }

        polygons
    }

    #[cfg(feature = "svg")]
    pub fn write_svg(&self, path: &str, c: Option<Coordinates>) -> Result<(), Box<dyn Error>> {
        use svg::node::element::Line;

        let polys = self.get_polygons();
        let areas = self.compute_areas();
        let max_distances = if let Some(coord) = c {
            self.compute_max_distances(coord)
                .into_iter()
                .map(Some)
                .collect()
        } else {
            vec![None; areas.len()]
        };

        let bounding_rect = polys.last().unwrap().bounding_rect().unwrap();
        let (min_x, min_y) = bounding_rect.min().x_y();
        let (max_x, max_y) = bounding_rect.max().x_y();
        let document = polys.into_iter().zip(areas).zip(max_distances).fold(
            Document::new().set(
                "viewBox",
                (
                    min_x / 100.0,
                    min_y / 100.0,
                    max_x / 100.0 - min_x / 100.0,
                    max_y / 100.0 - min_y / 100.0,
                ),
            ),
            |mut doc, ((pi, _area), dist)| {
                if let Some(coord) = c {
                    if let Some(((x, y), _)) = dist {
                        doc = doc.add(
                            Line::new()
                                .set("x1", x / 100.0)
                                .set("y1", (min_y + (max_y - y)) / 100.0)
                                .set("x2", coord.easting().unwrap() / 100.0)
                                .set("y2", (min_y + (max_y - coord.northing().unwrap())) / 100.0)
                                .set("stroke", "black"),
                        );
                    }
                }
                doc = pi.iter().fold(doc, |doc_nested, p| {
                    let points_ext = p
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

                    doc_nested.add(
                        SvgPolygon::new()
                            .set("fill", "none")
                            .set("stroke", "red")
                            .set("points", points_ext.join(" ")),
                    )
                });
                doc
            },
        );
        svg::save(path, &document)?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Isochrone {
    polygons: Vec<Vec<Coordinates>>,
    time_limit: u32, // In minutes.
}

impl Isochrone {
    pub fn new(polygons: Vec<Vec<Coordinates>>, time_limit: u32) -> Self {
        Self {
            polygons,
            time_limit,
        }
    }

    /// Transforms the isochrone polygons into geo::MultiPolygons to be able to use various
    /// functionalities of the crate. The polygons are in lv95 coordinates
    pub fn to_polygons(&self) -> MultiPolygon {
        self.polygons
            .iter()
            .map(|p| {
                Polygon::new(
                    LineString::from(
                        p.iter()
                            .map(|c| {
                                if let (Some(x), Some(y)) = (c.easting(), c.northing()) {
                                    (x, y)
                                } else {
                                    wgs84_to_lv95(c.latitude().unwrap(), c.longitude().unwrap())
                                }
                            })
                            .collect::<Vec<_>>(),
                    ),
                    vec![],
                )
            })
            .collect()
    }

    pub fn compute_area(&self) -> f64 {
        self.to_polygons().iter().map(|p| p.unsigned_area()).sum()
    }

    pub fn compute_max_distance(&self, c: Coordinates) -> ((f64, f64), f64) {
        self.to_polygons().iter().flat_map(|p| p.exterior()).fold(
            ((f64::MIN, f64::MIN), f64::MIN),
            |((o_x, o_y), max), coord| {
                let (c_x, c_y) = if let (Some(x), Some(y)) = (c.easting(), c.northing()) {
                    (x, y)
                } else {
                    wgs84_to_lv95(c.latitude().unwrap(), c.longitude().unwrap())
                };
                let dist = f64::sqrt(f64::powi(c_x - coord.x, 2) + f64::powi(c_y - coord.y, 2));
                if dist > max {
                    ((c_x, c_y), dist)
                } else {
                    ((o_x, o_y), max)
                }
            },
        )
    }
}

#[derive(Debug, EnumString, PartialEq, Clone, Copy)]
pub enum DisplayMode {
    #[strum(serialize = "circles")]
    Circles,
    #[strum(serialize = "contour_line")]
    ContourLine,
}
