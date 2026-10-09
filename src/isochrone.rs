mod circles;
mod contour_line;
pub(crate) mod externals;
mod models;
pub(crate) mod utils;

use std::{collections::HashMap, fmt, time::Instant};

use chrono::{Duration, NaiveDateTime};
use geo::{BooleanOps, MultiPolygon};
use hrdf_parser::{CoordinateSystem, Coordinates};
use orx_parallel::*;

use crate::{
    cli::{date_time, minutes},
    routing::{Router, compute_routes_from_origin, compute_routes_to_destination},
};
use contour_line::Grid;
use models::Isochrone;
pub use models::{DisplayMode as IsochroneDisplayMode, IsochroneMap};
use utils::{lv95, lv95_to_wgs84, walking_distance, wgs84_to_lv95};

/// Isochrones around a point: where one gets from it, or with `reverse` where one comes from
/// to reach it, within `time_limit` of `at`, every `interval`.
#[derive(clap::Args, Clone, Debug)]
pub struct IsochroneArgs {
    /// Latitude of the point
    #[arg(long, default_value_t = 46.20956654)]
    pub latitude: f64,
    /// Longitude of the point
    #[arg(long, default_value_t = 6.13536)]
    pub longitude: f64,
    /// Departure date and time (arrival in the reverse modes)
    #[arg(short = 'd', long = "departure-at", visible_alias = "arrival-at", short_alias = 'a', value_parser = date_time, default_value = "2025-04-10 15:36:00")]
    pub at: NaiveDateTime,
    /// Maximum time of the isochrone in minutes
    #[arg(short, long, value_parser = minutes, default_value = "60")]
    pub time_limit: Duration,
    /// Time interval between two isochrones in minutes
    #[arg(short, long, value_parser = minutes, default_value = "10")]
    pub interval: Duration,
    /// Maximum number of transfers
    #[arg(short, long, default_value_t = 10)]
    pub max_num_explorable_connections: i32,
    /// Number of stops within walking distance to start from
    #[arg(short, long, default_value_t = 5)]
    pub num_starting_points: usize,
    /// Verbose on or off
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(skip)]
    pub reverse: bool,
}

impl fmt::Display for IsochroneArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let at = if self.reverse {
            "arrival_at"
        } else {
            "departure_at"
        };
        write!(
            f,
            "longitude: {}, latitude: {}, {at}: {}, time_limit: {}, interval: {}",
            self.longitude, self.latitude, self.at, self.time_limit, self.interval
        )
    }
}

/// The isochrones of `args`.
pub fn compute_isochrones(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    args: &IsochroneArgs,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> IsochroneMap {
    let started = Instant::now();
    let data = reached(router, args, args.at, num_threads);
    let bounding_box = bounding_box(&data, args.time_limit);
    let grid = (display_mode == IsochroneDisplayMode::ContourLine)
        .then(|| Grid::new(&data, bounding_box, args.time_limit, num_threads));
    let isochrones = isochrones(args, excluded_polygons, |limit| match &grid {
        Some(grid) => grid.polygons(limit),
        None => circles::get_polygons(&data, limit, num_threads),
    });
    if args.verbose {
        log::info!("Isochrones for {args} in {:.2?}", started.elapsed());
    }
    map(isochrones, args, args.at, bounding_box)
}

/// The isochrones with the largest area for a time in [at - delta_time, at + delta_time).
pub fn compute_optimal_isochrones(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    args: &IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> IsochroneMap {
    pick(
        router,
        excluded_polygons,
        args,
        delta_time,
        display_mode,
        num_threads,
        |a, b| a > b,
    )
}

/// The isochrones with the smallest area for a time in [at - delta_time, at + delta_time).
pub fn compute_worst_isochrones(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    args: &IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
) -> IsochroneMap {
    pick(
        router,
        excluded_polygons,
        args,
        delta_time,
        display_mode,
        num_threads,
        |a, b| a < b,
    )
}

/// The isochrones of the time in the window whose largest area is `better` than the others'.
fn pick(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    args: &IsochroneArgs,
    delta_time: Duration,
    display_mode: IsochroneDisplayMode,
    num_threads: usize,
    better: fn(f64, f64) -> bool,
) -> IsochroneMap {
    let started = Instant::now();
    let best = window(args, delta_time)
        .into_par()
        .num_threads(num_threads)
        .map(|at| {
            let args = IsochroneArgs { at, ..args.clone() };
            compute_isochrones(
                router,
                excluded_polygons,
                &args,
                display_mode,
                inner_threads(num_threads),
            )
        })
        .reduce(|lhs, rhs| {
            if better(lhs.compute_max_area(), rhs.compute_max_area()) {
                lhs
            } else {
                rhs
            }
        });
    if args.verbose {
        log::info!(
            "Picked among the isochrones of {} times in {:.2?}",
            2 * delta_time.num_minutes(),
            started.elapsed()
        );
    }
    best.expect("The time window is empty")
}

/// Contour isochrones of the time to reach each point, averaged over the times in
/// [at - delta_time, at + delta_time).
pub fn compute_average_isochrones(
    router: &Router,
    excluded_polygons: &MultiPolygon,
    args: &IsochroneArgs,
    delta_time: Duration,
    num_threads: usize,
) -> IsochroneMap {
    let started = Instant::now();
    let data: Vec<_> = window(args, delta_time)
        .par()
        .num_threads(num_threads)
        .map(|&at| reached(router, args, at, inner_threads(num_threads)))
        .collect();
    let bounding_box = bounding_box(data.iter().flatten(), args.time_limit);
    let grids = data
        .iter()
        .map(|d| Grid::new(d, bounding_box, args.time_limit, num_threads))
        .collect();
    let average = Grid::average(grids);
    let isochrones = isochrones(args, excluded_polygons, |limit| average.polygons(limit));
    if args.verbose {
        log::info!("Average isochrones for {args} in {:.2?}", started.elapsed());
    }
    map(isochrones, args, args.at, bounding_box)
}

/// The times of the window around `args.at`, a minute apart.
fn window(args: &IsochroneArgs, delta_time: Duration) -> Vec<NaiveDateTime> {
    let minutes = delta_time.num_minutes();
    (-minutes..minutes)
        .map(|m| args.at + Duration::minutes(m))
        .collect()
}

/// Threads for a computation already running in parallel with others: one, or the global pool
/// (0) when that is what the outer computation uses.
pub(crate) fn inner_threads(num_threads: usize) -> usize {
    num_threads.min(1)
}

/// Every stop reached from (or reaching) the point at `at`, with its LV95 coordinates and the
/// shortest time from (or to) `at`.
fn reached(
    router: &Router,
    args: &IsochroneArgs,
    at: NaiveDateTime,
    num_threads: usize,
) -> Vec<(Coordinates, Duration)> {
    let routes = if args.reverse {
        compute_routes_to_destination
    } else {
        compute_routes_from_origin
    };
    let routes = routes(
        router,
        args.latitude,
        args.longitude,
        at,
        args.time_limit,
        args.num_starting_points,
        num_threads,
        args.max_num_explorable_connections,
        args.verbose,
    );
    let mut best: HashMap<i32, (Coordinates, Duration)> = HashMap::new();
    for route in &routes {
        let (stop_id, coordinates, duration) = if args.reverse {
            let first = &route.sections[0];
            (
                first.departure_stop_id,
                first.departure_stop_lv95,
                at - route.departure_at,
            )
        } else {
            let last = route.sections.last().expect("Route sections was empty");
            (
                last.arrival_stop_id,
                last.arrival_stop_lv95,
                route.arrival_at - at,
            )
        };
        if let Some(coordinates) = coordinates {
            let shortest = best.entry(stop_id).or_insert((coordinates, duration));
            shortest.1 = shortest.1.min(duration);
        }
    }
    best.into_values().collect()
}

/// The isochrones every `args.interval` up to `args.time_limit`, without the excluded areas.
fn isochrones(
    args: &IsochroneArgs,
    excluded_polygons: &MultiPolygon,
    polygons: impl Fn(Duration) -> MultiPolygon,
) -> Vec<Isochrone> {
    let count = args.time_limit.num_minutes() / args.interval.num_minutes();
    (1..=count)
        .map(|i| {
            let limit = Duration::minutes(args.interval.num_minutes() * i);
            Isochrone::new(
                polygons(limit).difference(excluded_polygons),
                limit.num_minutes() as u32,
            )
        })
        .collect()
}

/// The LV95 box around the stops and what can be walked from them in the time left.
fn bounding_box<'a>(
    data: impl IntoIterator<Item = &'a (Coordinates, Duration)>,
    time_limit: Duration,
) -> ((f64, f64), (f64, f64)) {
    let empty = (
        (f64::INFINITY, f64::INFINITY),
        (f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    data.into_iter()
        .fold(empty, |((x0, y0), (x1, y1)), (coordinates, duration)| {
            let ((e, n), walk) = (lv95(coordinates), walking_distance(time_limit - *duration));
            (
                (x0.min(e - walk), y0.min(n - walk)),
                (x1.max(e + walk), y1.max(n + walk)),
            )
        })
}

fn map(
    isochrones: Vec<Isochrone>,
    args: &IsochroneArgs,
    at: NaiveDateTime,
    ((x0, y0), (x1, y1)): ((f64, f64), (f64, f64)),
) -> IsochroneMap {
    let (easting, northing) = wgs84_to_lv95(args.latitude, args.longitude);
    let center = Coordinates::new(CoordinateSystem::LV95, easting, northing);
    let areas = isochrones.iter().map(|i| i.compute_area()).collect();
    let max_distances = (isochrones.iter())
        .map(|i| {
            let ((x, y), max) = i.compute_max_distance(center);
            (lv95_to_wgs84(x, y), max)
        })
        .collect();
    // ponytail: the point is stored as (longitude, latitude), unlike other WGS84 coordinates;
    // kept as is because the map frontend reads it that way.
    let point = Coordinates::new(CoordinateSystem::WGS84, args.longitude, args.latitude);
    let bounding_box = (lv95_to_wgs84(x0, y0), lv95_to_wgs84(x1, y1));
    IsochroneMap::new(isochrones, areas, max_distances, point, at, bounding_box)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Area, Contains, Point};
    use std::f64::consts::PI;
    use test_log::test;

    fn area_m2(polygons: &MultiPolygon) -> f64 {
        utils::multi_polygon_to_lv95(polygons)
            .iter()
            .map(|p| p.unsigned_area())
            .sum()
    }

    fn point(e: f64, n: f64, minutes: i64) -> (Coordinates, Duration) {
        (
            Coordinates::new(CoordinateSystem::LV95, e, n),
            Duration::minutes(minutes),
        )
    }

    fn hexagon_area(radius: f64) -> f64 {
        (3.0 * 3.0_f64.sqrt() / 2.0) * radius * radius
    }

    #[test]
    fn a_single_stop_gives_a_hexagon_growing_with_time() {
        let data = [point(2600000.0, 1200000.0, 0)];
        let area = |minutes| area_m2(&circles::get_polygons(&data, Duration::minutes(minutes), 1));
        let expected = hexagon_area(walking_distance(Duration::minutes(30)));
        assert!((area(30) - expected).abs() / expected < 1e-4);
        assert!(area(20) < area(30) && area(30) < area(40));
    }

    #[test]
    fn circles_cover_the_stops_reached_in_time_only() {
        let data = [
            point(2600000.0, 1200000.0, 5),
            point(2610000.0, 1200000.0, 35),
            point(2600500.0, 1200500.0, 15),
        ];
        let result = circles::get_polygons(&data, Duration::minutes(30), 1);
        let contains = |e, n| {
            let (lat, lon) = lv95_to_wgs84(e, n);
            result.contains(&Point::new(lat, lon))
        };
        assert!(contains(2600000.0, 1200000.0) && contains(2600500.0, 1200500.0));
        assert!(!contains(2610000.0, 1200000.0));
        assert_eq!(result.0.len(), 1);
    }

    #[test]
    fn overlapping_circles_merge() {
        let data: Vec<_> = (0..50)
            .map(|i| point(2600000.0 + i as f64 * 100.0, 1200000.0, 5))
            .collect();
        let result = circles::get_polygons(&data, Duration::minutes(30), 1);
        assert_eq!(result.0.len(), 1);
        let area = area_m2(&result);
        assert!(area > 0.0 && area < 50.0 * hexagon_area(walking_distance(Duration::minutes(25))));
    }

    #[test]
    fn contour_lies_between_the_four_and_six_minute_circles() {
        // Stops every 10 m, reached as if walking from the center.
        let meters_per_minute = utils::WALKING_SPEED_IN_KILOMETERS_PER_HOUR * 1000.0 / 60.0;
        let data: Vec<_> = (-50..50)
            .flat_map(|i| (-50..50).map(move |j| (i as f64 * 10.0, j as f64 * 10.0)))
            .map(|(de, dn)| {
                point(
                    2600000.0 + de,
                    1200000.0 + dn,
                    (de.hypot(dn) / meters_per_minute) as i64,
                )
            })
            .collect();
        let grid = Grid::new(
            &data,
            bounding_box(&data, Duration::minutes(10)),
            Duration::minutes(10),
            1,
        );
        let area = area_m2(&grid.polygons(Duration::minutes(5)));
        let circle = |minutes| PI * walking_distance(Duration::minutes(minutes)).powi(2);
        assert!(circle(4) < area && area < circle(6), "{area}");
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn circles_and_contour_cover_the_same_area() {
        let router = Router::new(hrdf_parser::Timetable::year(2025).load().await.unwrap());
        let at = NaiveDateTime::parse_from_str("2025-06-15 12:10", "%Y-%m-%d %H:%M").unwrap();
        let time_limit = Duration::minutes(60);
        let args = IsochroneArgs {
            latitude: 47.3769, // Zürich
            longitude: 8.5417,
            at,
            time_limit,
            interval: time_limit,
            max_num_explorable_connections: 10,
            num_starting_points: 10,
            verbose: false,
            reverse: false,
        };
        let data = reached(&router, &args, at, 8);
        let circles = area_m2(&circles::get_polygons(&data, time_limit, 1));
        let grid = Grid::new(&data, bounding_box(&data, time_limit), time_limit, 1);
        let contour = area_m2(&grid.polygons(time_limit));
        assert!(
            (circles - contour).abs() / contour < 0.01,
            "circles {circles}, contour {contour}"
        );
    }
}
