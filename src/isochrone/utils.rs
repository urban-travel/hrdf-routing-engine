use std::f64::consts::PI;

use chrono::Duration;
use hrdf_parser::Coordinates;

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

pub fn distance_between_2_points(point1: Coordinates, point2: Coordinates) -> f64 {
    let x_sqr = (point2.easting() - point1.easting()).powi(2);
    let y_sqr = (point2.northing() - point1.northing()).powi(2);
    (x_sqr + y_sqr).sqrt()
}

pub fn distance_to_time(distance: f64, speed_in_kilometers_per_hour: f64) -> Duration {
    let speed_in_meters_per_second = speed_in_kilometers_per_hour / 3.6;
    Duration::seconds((distance / speed_in_meters_per_second) as i64)
}

pub fn time_to_distance(duration: Duration, speed_in_kilometers_per_hour: f64) -> f64 {
    let speed_in_meters_per_second = speed_in_kilometers_per_hour / 3.6;
    duration.num_seconds() as f64 * speed_in_meters_per_second
}

fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * PI / 180.0
}

pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let radius_of_earth_km = 6371.0;

    let lat1_rad = degrees_to_radians(lat1);
    let lon1_rad = degrees_to_radians(lon1);
    let lat2_rad = degrees_to_radians(lat2);
    let lon2_rad = degrees_to_radians(lon2);

    let delta_lat = lat2_rad - lat1_rad;
    let delta_lon = lon2_rad - lon1_rad;

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    radius_of_earth_km * c
}
