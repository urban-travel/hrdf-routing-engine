use geo::{LineString, Polygon};
use hrdf_parser::Coordinates;
use serde::Serialize;
use strum_macros::EnumString;

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

    pub fn isochrones(&self) -> &Vec<Isochrone> {
        &self.isochrones
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

    // Geo polygons are a line which is its exterior part and can have holes in them.
    // Each isochrone also has this information encoded and is the isochrone of level
    // i with "holes" from isochrones of level i-1.
    // Therefore the isocrhone[i-i] will be the interior line of the line of interior[i].
    pub fn to_polygons(&self) -> Vec<Polygon> {
        let exterior_only = self
            .polygons
            .iter()
            .map(|p| {
                Polygon::new(
                    LineString::from(
                        p.iter()
                            .map(|c| {
                                (
                                    c.easting().expect("Wrong coordinate system"),
                                    c.northing().expect("Wrong coordinate system"),
                                )
                            })
                            .collect::<Vec<_>>(),
                    ),
                    vec![],
                )
            })
            .collect::<Vec<_>>();
        // for i in 0..exterior_only.len() - 1 {
        //     let interior = exterior_only[i].exterior().clone();
        //     exterior_only[i + 1].interiors_push(interior);
        // }

        exterior_only
    }

    // pub fn compute_area(&self) -> Vec<f64> {
    //     self.to_polygons().map(|p| p)
    // }
}

#[derive(Debug, EnumString, PartialEq)]
pub enum DisplayMode {
    #[strum(serialize = "circles")]
    Circles,
    #[strum(serialize = "contour_line")]
    ContourLine,
}
