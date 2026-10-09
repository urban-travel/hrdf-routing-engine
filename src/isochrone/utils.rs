use std::f64::consts::PI;

use chrono::Duration;
use geo::{LineString, MultiPolygon, Polygon};
use hrdf_parser::{Coordinates, Stop};

pub const WALKING_SPEED_IN_KILOMETERS_PER_HOUR: f64 = 4.0;

/// https://github.com/antistatique/swisstopo
#[rustfmt::skip]
pub fn lv95_to_wgs84(easting: f64, northing: f64) -> (f64, f64) {
    let y_aux = (easting - 2600000.0) / 1000000.0;
    let x_aux = (northing - 1200000.0) / 1000000.0;

    // Latitude calculation.
    let latitude = 16.9023892
        + 3.238272 * x_aux
        - 0.270978 * y_aux.powi(2)
        - 0.002528 * x_aux.powi(2)
        - 0.0447 * y_aux.powi(2) * x_aux
        - 0.0140 * x_aux.powi(3);
    let latitude = latitude * 100.0 / 36.0;

    // Longitude calculation.
    let longitude = 2.6779094
        + 4.728982 * y_aux
        + 0.791484 * y_aux * x_aux
        + 0.1306 * y_aux * x_aux.powi(2)
        - 0.0436 * y_aux.powi(3);
    let longitude = longitude * 100.0 / 36.0;

    (latitude, longitude)
}

/// https://github.com/antistatique/swisstopo
#[rustfmt::skip]
pub fn wgs84_to_lv95(latitude: f64, longitude: f64) -> (f64, f64) {
    let latitude = deg_to_sex(latitude);
    let longitude = deg_to_sex(longitude);

    let phi = deg_to_sec(latitude);
    let lambda  = deg_to_sec(longitude);

    let phi_aux = (phi - 169028.66) / 10000.0;
    let lambda_aux =  (lambda - 26782.5) / 10000.0;

    // Easting calculation.
    let easting = 2600072.37
        + 211455.93 * lambda_aux
        - 10938.51 * lambda_aux * phi_aux
        - 0.36 * lambda_aux * phi_aux.powi(2)
        - 44.54 * lambda_aux.powi(3);

    // Northing calculation.
    let northing =  1200147.07
        + 308807.95 * phi_aux
        + 3745.25 * lambda_aux.powi(2)
        + 76.63 * phi_aux.powi(2)
        - 194.56 * lambda_aux.powi(2) * phi_aux
        + 119.79 * phi_aux.powi(3);

    (easting, northing)
}

/// https://github.com/antistatique/swisstopo
fn deg_to_sex(angle: f64) -> f64 {
    let deg = angle as i64;
    let min = ((angle - deg as f64) * 60.0) as i64;
    let sec = (((angle - deg as f64) * 60.0) - min as f64) * 60.0;

    deg as f64 + min as f64 / 100.0 + sec / 10000.0
}

/// https://github.com/antistatique/swisstopo
fn deg_to_sec(angle: f64) -> f64 {
    let deg = angle as i64;
    let min = ((angle - deg as f64) * 100.0) as i64;
    let sec = (((angle - deg as f64) * 100.0) - min as f64) * 100.0;

    sec + min as f64 * 60.0 + deg as f64 * 3600.0
}

/// The (easting, northing) of LV95 coordinates.
pub fn lv95(c: &Coordinates) -> (f64, f64) {
    (
        c.easting().expect("LV95 coordinates"),
        c.northing().expect("LV95 coordinates"),
    )
}

/// The polygons, in WGS84 (latitude, longitude), converted to LV95.
pub fn multi_polygon_to_lv95(mp: &MultiPolygon) -> MultiPolygon {
    let to_lv95 = |ls: &LineString| {
        ls.coords()
            .map(|c| wgs84_to_lv95(c.x, c.y))
            .collect::<LineString>()
    };
    mp.iter()
        .map(|p| {
            Polygon::new(
                to_lv95(p.exterior()),
                p.interiors().iter().map(to_lv95).collect(),
            )
        })
        .collect()
}

pub fn walking_time(meters: f64) -> Duration {
    Duration::seconds((meters / (WALKING_SPEED_IN_KILOMETERS_PER_HOUR / 3.6)) as i64)
}

pub fn walking_distance(duration: Duration) -> f64 {
    duration.num_seconds() as f64 * (WALKING_SPEED_IN_KILOMETERS_PER_HOUR / 3.6)
}

/// Great-circle distance in kilometers.
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radians = |degrees: f64| degrees * PI / 180.0;
    let (lat1, lon1, lat2, lon2) = (radians(lat1), radians(lon1), radians(lat2), radians(lon2));
    let a = ((lat2 - lat1) / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * ((lon2 - lon1) / 2.0).sin().powi(2);
    6371.0 * (2.0 * a.sqrt().atan2((1.0 - a).sqrt()))
}

/// Time to walk in a straight line between a point and a stop.
pub fn walking_time_to(latitude: f64, longitude: f64, stop: &Stop) -> Duration {
    let c = stop.wgs84.expect("stop with WGS84 coordinates");
    let km = haversine_distance(
        latitude,
        longitude,
        c.latitude().unwrap(),
        c.longitude().unwrap(),
    );
    walking_time(km * 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgs84_to_lv95_matches_known_points() {
        // (latitude, longitude, easting, northing, tolerance in meters)
        let points = [
            (46.9479, 7.4474, 2600555.0, 1199646.0, 1000.0), // Bern cathedral
            (47.3769, 8.5417, 2683074.0, 1247950.0, 500.0),  // Zürich HB
            (46.2044, 6.1432, 2500000.0, 1118000.0, 1000.0), // Geneva
            (46.0037, 8.9511, 2717900.0, 1095900.0, 1000.0), // Lugano
        ];
        for (lat, lon, e, n, tolerance) in points {
            let (easting, northing) = wgs84_to_lv95(lat, lon);
            assert!(
                (easting - e).abs() < tolerance && (northing - n).abs() < tolerance,
                "{lat}, {lon}"
            );
            let (lat2, lon2) = lv95_to_wgs84(easting, northing);
            assert!(
                (lat - lat2).abs() < 1e-3 && (lon - lon2).abs() < 1e-3,
                "round trip of {lat}, {lon}"
            );
        }
    }

    #[test]
    fn haversine_distance_matches_known_distances() {
        // (from, to, km, tolerance)
        let cases = [
            ((46.9479, 7.4474), (47.3769, 8.5417), 95.0, 5.0), // Bern - Zürich
            ((46.2044, 6.1432), (46.0037, 8.9511), 217.0, 10.0), // Geneva - Lugano
            ((46.9479, 7.4474), (46.2044, 6.1432), 129.0, 5.0), // Bern - Geneva
            ((46.9479, 7.4474), (46.9579, 7.4574), 1.35, 0.2),
            ((46.9479, 7.4474), (46.9479, 7.4474), 0.0, 0.0),
        ];
        for ((lat1, lon1), (lat2, lon2), km, tolerance) in cases {
            let distance = haversine_distance(lat1, lon1, lat2, lon2);
            assert!(
                (distance - km).abs() <= tolerance,
                "{distance} km instead of {km}"
            );
        }
    }

    #[test]
    fn walking_is_four_kilometers_per_hour() {
        assert_eq!(walking_distance(Duration::minutes(60)), 4000.0);
        assert_eq!(walking_time(4000.0), Duration::minutes(60));
        assert_eq!(walking_time(0.0), Duration::zero());
        for meters in [100.0, 1000.0, 2500.0] {
            assert!((walking_distance(walking_time(meters)) - meters).abs() <= 1.2);
        }
    }
}
