use std::f64::consts::PI;

use chrono::{Duration, NaiveDateTime};
use geo::{LineString, MultiPolygon, Polygon};
use hrdf_parser::{Coordinates, Stop};

use super::constants::WALKING_SPEED_IN_KILOMETERS_PER_HOUR;

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

/// Creates a new MultiPolygon in lv95 coordinates. We suppose the original polygon was in wgs84
/// coordinates
pub fn multi_polygon_to_lv95(mp: &MultiPolygon) -> MultiPolygon {
    mp.iter()
        .map(|p| {
            let exterior = LineString::from(
                p.exterior()
                    .coords()
                    .map(|c| wgs84_to_lv95(c.x, c.y))
                    .collect::<Vec<_>>(),
            );
            let interiors = p
                .interiors()
                .iter()
                .map(|ls| ls.coords().map(|c| wgs84_to_lv95(c.x, c.y)).collect())
                .collect();
            Polygon::new(exterior, interiors)
        })
        .collect()
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
    let x_sqr = (point2.easting().expect("Wrong coordinate system")
        - point1.easting().expect("Wrong coordinate system"))
    .powi(2);
    let y_sqr = (point2.northing().expect("Wrong coordinate system")
        - point1.northing().expect("Wrong coordinate system"))
    .powi(2);
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

/// Adjusts the departure time from a stop, given the person is walking from long/lat to stop
pub fn adjust_departure_at(
    departure_at: NaiveDateTime,
    time_limit: Duration,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_stop: &Stop,
) -> (NaiveDateTime, Duration) {
    let distance = {
        let coord = departure_stop.wgs84_coordinates().unwrap();

        haversine_distance(
            origin_point_latitude,
            origin_point_longitude,
            coord.latitude().expect("Wrong coordinate system"),
            coord.longitude().expect("Wrong coordinate system"),
        ) * 1000.0
    };

    let duration = distance_to_time(distance, WALKING_SPEED_IN_KILOMETERS_PER_HOUR);

    let adjusted_departure_at = departure_at.checked_add_signed(duration).unwrap();
    let adjusted_time_limit = time_limit - duration;

    (adjusted_departure_at, adjusted_time_limit)
}

#[derive(Debug, Clone, Copy)]
pub struct NaiveDateTimeRange {
    from: NaiveDateTime,
    to: NaiveDateTime,
    incr: Duration,
}

impl NaiveDateTimeRange {
    pub fn new(from: NaiveDateTime, to: NaiveDateTime, incr: Duration) -> Self {
        Self { from, to, incr }
    }
}

impl Iterator for NaiveDateTimeRange {
    type Item = NaiveDateTime;
    fn next(&mut self) -> Option<Self::Item> {
        if self.from > self.to {
            return None;
        }
        let maybe_next = self.from + self.incr;
        self.from = maybe_next;
        (self.from < self.to).then_some(maybe_next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wgs84_to_lv95_bern_cathedral() {
        // Bern Cathedral: 46.9479°N, 7.4474°E
        // Expected LV95: approximately 2600555, 1199646
        let (easting, northing) = wgs84_to_lv95(46.9479, 7.4474);

        // Allow 1000m tolerance (LV95 conversion can vary by implementation)
        assert!(
            (easting - 2600555.0).abs() < 1000.0,
            "Easting was {}, expected ~2600555",
            easting
        );
        assert!(
            (northing - 1199646.0).abs() < 1000.0,
            "Northing was {}, expected ~1199646",
            northing
        );
    }

    #[test]
    fn test_wgs84_to_lv95_zurich_hb() {
        // Zürich HB: 47.3769°N, 8.5417°E
        // Expected LV95: approximately 2683074, 1247950
        let (easting, northing) = wgs84_to_lv95(47.3769, 8.5417);

        assert!(
            (easting - 2683074.0).abs() < 500.0,
            "Easting was {}, expected ~2683074",
            easting
        );
        assert!(
            (northing - 1247950.0).abs() < 500.0,
            "Northing was {}, expected ~1247950",
            northing
        );
    }

    #[test]
    fn test_wgs84_to_lv95_geneva() {
        // Geneva: 46.2044°N, 6.1432°E
        let (easting, northing) = wgs84_to_lv95(46.2044, 6.1432);

        assert!(
            (easting - 2500000.0).abs() < 1000.0,
            "Easting was {}, expected ~2500000",
            easting
        );
        assert!(
            (northing - 1118000.0).abs() < 1000.0,
            "Northing was {}, expected ~1118000",
            northing
        );
    }

    #[test]
    fn test_wgs84_to_lv95_lugano() {
        // Lugano: 46.0037°N, 8.9511°E
        let (easting, northing) = wgs84_to_lv95(46.0037, 8.9511);

        assert!(
            (easting - 2717900.0).abs() < 1000.0,
            "Easting was {}, expected ~2717900",
            easting
        );
        assert!(
            (northing - 1095900.0).abs() < 1000.0,
            "Northing was {}, expected ~1095900",
            northing
        );
    }

    #[test]
    fn test_lv95_to_wgs84_round_trip() {
        // Test various Swiss locations for round-trip accuracy
        let test_points = vec![
            (46.9479, 7.4474), // Bern
            (47.3769, 8.5417), // Zürich
            (46.2044, 6.1432), // Geneva
            (46.5197, 6.6323), // Lausanne
            (46.0037, 8.9511), // Lugano
            (47.5596, 7.5886), // Basel
        ];

        for (lat, lon) in test_points {
            let (e, n) = wgs84_to_lv95(lat, lon);
            let (lat2, lon2) = lv95_to_wgs84(e, n);

            assert!(
                (lat - lat2).abs() < 0.0001,
                "Latitude mismatch: {} -> {} (diff: {})",
                lat,
                lat2,
                (lat - lat2).abs()
            );
            assert!(
                (lon - lon2).abs() < 0.0001,
                "Longitude mismatch: {} -> {} (diff: {})",
                lon,
                lon2,
                (lon - lon2).abs()
            );
        }
    }

    #[test]
    fn test_haversine_distance_bern_zurich() {
        // Bern to Zürich: approximately 94.5 km
        let distance = haversine_distance(46.9479, 7.4474, 47.3769, 8.5417);

        // Convert to meters
        assert!(
            (distance - 95.5).abs() < 0.1,
            "Distance should be ~95.5 km, got {} km",
            distance
        );
    }

    #[test]
    fn test_haversine_distance_same_point() {
        let distance = haversine_distance(46.9479, 7.4474, 46.9479, 7.4474);
        assert_eq!(distance, 0.0, "Distance to same point should be 0");
    }

    #[test]
    fn test_haversine_distance_geneva_lugano() {
        // Geneva to Lugano: approximately 218 km (as the crow flies)
        let distance = haversine_distance(46.2044, 6.1432, 46.0037, 8.9511);

        assert!(
            (distance - 217.6).abs() < 0.1,
            "Distance should be ~217.6 km, got {} km",
            distance
        );
    }

    #[test]
    fn test_haversine_distance_bern_geneva() {
        // Bern to Geneva: approximately 130 km (as the crow flies)
        let distance = haversine_distance(46.9479, 7.4474, 46.2044, 6.1432);

        assert!(
            (distance - 129.5).abs() < 0.1,
            "Distance should be ~129.5 km, got {} km",
            distance
        );
    }

    #[test]
    fn test_haversine_distance_small_distance() {
        // Two very close points (about 1 km apart)
        let distance = haversine_distance(46.9479, 7.4474, 46.9579, 7.4574);

        assert!(
            distance < 1.347,
            "Small distance should be < 1.347 km, but is {}",
            distance
        );
        assert!(
            distance > 1.345,
            "Small distance should be > 1.345 km, but is {}",
            distance
        );
    }

    #[test]
    fn test_time_to_distance_at_5_kmh() {
        assert_eq!(time_to_distance(Duration::minutes(60), 5.0), 5000.0);
        assert_eq!(time_to_distance(Duration::minutes(30), 5.0), 2500.0);
    }

    #[test]
    fn test_time_to_distance_different_speeds() {
        let distance = time_to_distance(Duration::minutes(60), 3.0);
        assert!(
            (distance - 3000.0).abs() < 0.1,
            "Expected ~3000.0, got {}",
            distance
        );
        let distance = time_to_distance(Duration::minutes(60), 6.0);
        assert!(
            (distance - 6000.0).abs() < 0.1,
            "Expected ~6000.0, got {}",
            distance
        );
    }

    #[test]
    fn test_time_to_distance_zero() {
        assert_eq!(time_to_distance(Duration::minutes(0), 5.0), 0.0);
        assert_eq!(time_to_distance(Duration::seconds(0), 5.0), 0.0);
    }

    #[test]
    fn test_distance_to_time() {
        assert_eq!(distance_to_time(5000.0, 5.0).num_minutes(), 60);
        assert_eq!(distance_to_time(2500.0, 5.0).num_minutes(), 30);
    }

    #[test]
    fn test_distance_to_time_zero() {
        assert_eq!(distance_to_time(0.0, 5.0).num_seconds(), 0);
    }

    #[test]
    fn test_time_distance_round_trip() {
        let test_distances = vec![100.0, 500.0, 1000.0, 2500.0, 5000.0];
        for distance in test_distances {
            let time = distance_to_time(distance, 5.0);
            let distance2 = time_to_distance(time, 5.0);

            assert!(
                (distance - distance2).abs() < 0.01,
                "Round trip failed: {} -> {} seconds -> {}",
                distance,
                time.num_seconds(),
                distance2
            );
        }
    }

    #[test]
    fn test_degrees_to_radians() {
        // Test common angles
        assert!((degrees_to_radians(0.0) - 0.0).abs() < 0.0001);
        assert!((degrees_to_radians(90.0) - PI / 2.0).abs() < 0.0001);
        assert!((degrees_to_radians(180.0) - PI).abs() < 0.0001);
        assert!((degrees_to_radians(360.0) - 2.0 * PI).abs() < 0.0001);
    }

    #[test]
    fn test_distance_between_2_points() {
        use hrdf_parser::CoordinateSystem;
        let point1 = Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0);
        let point2 = Coordinates::new(CoordinateSystem::LV95, 2601000.0, 1200000.0);
        let distance = distance_between_2_points(point1, point2);
        assert_eq!(distance, 1000.0);
    }

    #[test]
    fn test_distance_between_2_points_diagonal() {
        use hrdf_parser::CoordinateSystem;
        let point1 = Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0);
        let point2 = Coordinates::new(CoordinateSystem::LV95, 2600300.0, 1200400.0);
        let distance = distance_between_2_points(point1, point2);
        assert_eq!(distance, 500.0);
    }

    #[test]
    fn test_distance_between_2_points_same() {
        use hrdf_parser::CoordinateSystem;
        let point1 = Coordinates::new(CoordinateSystem::LV95, 2600000.0, 1200000.0);
        let distance = distance_between_2_points(point1, point1);
        assert_eq!(distance, 0.0);
    }

    #[test]
    fn test_naive_date_time_range_iteration() {
        use chrono::NaiveDateTime;

        let start =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let end =
            NaiveDateTime::parse_from_str("2025-06-15 10:05:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let range = NaiveDateTimeRange::new(start, end, Duration::minutes(1));
        let times: Vec<_> = range.collect();
        assert_eq!(times.len(), 4);

        let expected = vec![
            NaiveDateTime::parse_from_str("2025-06-15 10:01:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            NaiveDateTime::parse_from_str("2025-06-15 10:02:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            NaiveDateTime::parse_from_str("2025-06-15 10:03:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            NaiveDateTime::parse_from_str("2025-06-15 10:04:00", "%Y-%m-%d %H:%M:%S").unwrap(),
        ];

        assert_eq!(times, expected);
    }

    #[test]
    fn test_naive_date_time_range_empty() {
        use chrono::NaiveDateTime;

        let start =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let end =
            NaiveDateTime::parse_from_str("2025-06-15 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let range = NaiveDateTimeRange::new(start, end, Duration::minutes(1));
        let times: Vec<_> = range.collect();

        // Should be empty when start > end
        assert_eq!(times.len(), 0);
    }

    #[test]
    fn test_naive_date_time_range_single_step() {
        use chrono::NaiveDateTime;

        let start =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let end =
            NaiveDateTime::parse_from_str("2025-06-15 10:01:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let range = NaiveDateTimeRange::new(start, end, Duration::minutes(1));
        let times: Vec<_> = range.collect();
        assert_eq!(times.len(), 0);
    }
}
