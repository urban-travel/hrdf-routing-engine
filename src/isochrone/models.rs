use chrono::NaiveDateTime;
use geo::{Area, Contains, MultiPolygon};
use hrdf_parser::Coordinates;
use serde::Serialize;
use strum_macros::EnumString;

#[cfg(feature = "svg")]
use geo::BoundingRect;
#[cfg(feature = "svg")]
use std::error::Error;
use std::fmt::Display;
#[cfg(feature = "svg")]
use svg::Document;
#[cfg(feature = "svg")]
use svg::node::element::Polygon as SvgPolygon;

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

    /// Computes the area of the higher isochrone
    pub fn compute_max_area(&self) -> f64 {
        self.compute_areas().iter().fold(
            f64::MIN,
            |max_area, area| if *area > max_area { *area } else { max_area },
        )
    }

    pub fn get_polygons(&self) -> Vec<MultiPolygon> {
        let mut polygons = self
            .isochrones
            .iter()
            .map(|i| i.polygons().clone())
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

    pub fn departure_at(&self) -> NaiveDateTime {
        self.departure_at
    }

    #[cfg(feature = "svg")]
    pub fn write_svg(
        &self,
        path: &str,
        scale_factor: f64,
        c: Option<Coordinates>,
    ) -> Result<(), Box<dyn Error>> {
        const HEXES: [&str; 6] = [
            "#36AB68", // Nearest.
            "#91CF60", //
            "#D7FF67", //
            "#FFD767", //
            "#FC8D59", //
            "#E2453C", // Furthest.
        ];
        use svg::node::element::Line;

        let polys = self
            .get_polygons()
            .into_iter()
            .map(|m| multi_polygon_to_lv95(&m))
            .collect::<Vec<_>>();
        let areas = self.compute_areas();
        let max_distances = if let Some(coord) = c {
            self.compute_max_distances(coord)
                .into_iter()
                .map(Some)
                .collect()
        } else {
            vec![None; areas.len()]
        };

        let bounding_rect = polys
            .last()
            .expect("MultiPolygons Vec is empty")
            .bounding_rect()
            .expect("Unable to find bounding rectangle");
        let (min_x, min_y) = bounding_rect.min().x_y();
        let (max_x, max_y) = bounding_rect.max().x_y();
        let mut document = polys
            .into_iter()
            .rev()
            .enumerate()
            .zip(areas.into_iter().rev())
            .fold(
                Document::new().set(
                    "viewBox",
                    (
                        min_x * scale_factor,
                        min_y * scale_factor,
                        (max_x - min_x) * scale_factor,
                        (max_y - min_y) * scale_factor,
                    ),
                ),
                |mut doc, ((num, pi), _area)| {
                    doc = pi.iter().fold(doc, |doc_nested, p| {
                        let points_ext = p
                            .exterior()
                            .coords()
                            .map(|coord| {
                                format!(
                                    "{},{}",
                                    coord.x * scale_factor,
                                    (min_y + (max_y - coord.y)) * scale_factor
                                )
                            })
                            .collect::<Vec<_>>();

                        doc_nested.add(
                            SvgPolygon::new()
                                .set("fill", HEXES[num])
                                .set("stroke", "black")
                                .set("points", points_ext.join(" ")),
                        )
                    });
                    doc
                },
            );
        document = max_distances
            .into_iter()
            .rev()
            .fold(document, |mut doc, dist| {
                if let Some(coord) = c {
                    if let Some(((x, y), _)) = dist {
                        doc = doc.add(
                            Line::new()
                                .set("x1", x * scale_factor)
                                .set("y1", (min_y + (max_y - y)) * scale_factor)
                                .set("x2", coord.easting().unwrap() * scale_factor)
                                .set(
                                    "y2",
                                    (min_y + (max_y - coord.northing().unwrap())) * scale_factor,
                                )
                                .set("stroke", "black"),
                        );
                        doc
                    } else {
                        doc
                    }
                } else {
                    doc
                }
            });
        svg::save(path, &document)?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Isochrone {
    polygons: MultiPolygon,
    time_limit: u32, // In minutes.
}

impl Isochrone {
    pub fn new(polygons: MultiPolygon, time_limit: u32) -> Self {
        Self {
            polygons,
            time_limit,
        }
    }

    /// Transforms the isochrone polygons into geo::MultiPolygons to be able to use various
    /// functionalities of the crate. The polygons are in lv95 coordinates
    pub fn polygons(&self) -> &MultiPolygon {
        &self.polygons
    }

    pub fn compute_area(&self) -> f64 {
        multi_polygon_to_lv95(self.polygons())
            .iter()
            .map(|p| p.unsigned_area())
            .sum()
    }

    /// Computes the max distance from all the points in the isochrone to the c Coord.
    /// The distance is given in meters and the position in LV95 coordinates
    pub fn compute_max_distance(&self, c: Coordinates) -> ((f64, f64), f64) {
        self.polygons().iter().flat_map(|p| p.exterior()).fold(
            ((f64::MIN, f64::MIN), f64::MIN),
            |((o_x, o_y), max), coord| {
                let (cx_lv95, cy_lv95) = wgs84_to_lv95(coord.x, coord.y);
                let (c_x, c_y) = if let (Some(x), Some(y)) = (c.easting(), c.northing()) {
                    (x, y)
                } else {
                    wgs84_to_lv95(c.latitude().unwrap(), c.longitude().unwrap())
                };
                let dist = f64::sqrt(f64::powi(c_x - cx_lv95, 2) + f64::powi(c_y - cy_lv95, 2));
                if dist > max {
                    ((cx_lv95, cy_lv95), dist)
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

impl Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Circles => write!(f, "circles"),
            Self::ContourLine => write!(f, "contour_line"),
        }
    }
}
