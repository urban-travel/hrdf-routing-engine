use geo::{Area, Contains, LineString, MultiPolygon, Polygon};
use hrdf_parser::Coordinates;
use serde::Serialize;
use strum_macros::EnumString;

#[cfg(feature = "svg")]
use geo::BoundingRect;
#[cfg(feature = "svg")]
use std::error::Error;
#[cfg(feature = "svg")]
use svg::node::element::Polygon as SvgPolygon;
#[cfg(feature = "svg")]
use svg::Document;

use super::utils::wgs84_to_lv95;

#[derive(Debug, Serialize)]
pub struct IsochroneMap {
    isochrones: Vec<Isochrone>,
    departure_stop_coord: Coordinates,
    bounding_box: ((f64, f64), (f64, f64)),
}

impl IsochroneMap {
    pub fn new(
        isochrones: Vec<Isochrone>,
        departure_stop_coord: Coordinates,
        bounding_box: ((f64, f64), (f64, f64)),
    ) -> Self {
        Self {
            isochrones,
            departure_stop_coord,
            bounding_box,
        }
    }

    pub fn compute_areas(&self) -> Vec<f64> {
        self.isochrones.iter().map(|i| i.compute_area()).collect()
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
    pub fn write_svg(&self, path: &str) -> Result<(), Box<dyn Error>> {
        let polys = self.get_polygons();

        let bounding_rect = polys.last().unwrap().bounding_rect().unwrap();
        let (min_x, min_y) = bounding_rect.min().x_y();
        let (max_x, max_y) = bounding_rect.max().x_y();
        let document = polys.iter().fold(
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
        svg::save(format!("{path}"), &document)?;
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
    /// functionalities of the crate
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
}

#[derive(Debug, EnumString, PartialEq, Clone, Copy)]
pub enum DisplayMode {
    #[strum(serialize = "circles")]
    Circles,
    #[strum(serialize = "contour_line")]
    ContourLine,
}
