//! Journey planning with RAPTOR. A query goes through:
//!
//! 1. [`Router::day`]: the network of the query's date (`network.rs`), built on first use, where
//!    trips are grouped into patterns. Its reversed copy serves backward searches.
//! 2. `raptor::run`: the search (`raptor.rs`), with the minimum transfer times of `transfer.rs`.
//! 3. `Search::legs` then [`Route::new`] (`route.rs`): the journey found, as rides and walks.

mod network;
mod raptor;
mod route;
mod transfer;

use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Duration, NaiveDate, NaiveDateTime};
use hrdf_parser::{CoordinateSystem, Coordinates, Stop, Timetable};
use rustc_hash::FxHashMap;

use crate::isochrone::utils::{walking_time_to, wgs84_to_lv95};
use network::{Network, Trip};
use raptor::{INF, Leg, Search};
pub use route::{Route, RouteSection};
use transfer::TransferRules;

/// A timetable prepared for routing. The networks of the last dates queried are kept.
pub struct Router {
    pub timetable: Timetable,
    stop_ids: Vec<i32>,
    stop_index: FxHashMap<i32, u32>,
    footpaths_out: Vec<Vec<(u32, i16)>>,
    footpaths_in: Vec<Vec<(u32, i16)>>,
    transfers: TransferRules,
    days: Mutex<Vec<Arc<Day>>>,
}

struct Day {
    forward: Network,
    backward: OnceLock<Network>,
}

impl Day {
    fn date(&self) -> NaiveDate {
        self.forward.date
    }

    fn backward(&self) -> &Network {
        self.backward.get_or_init(|| self.forward.reversed())
    }
}

impl Router {
    pub fn new(timetable: Timetable) -> Self {
        let mut stop_ids: Vec<i32> = timetable.stops.keys().copied().collect();
        stop_ids.sort_unstable();
        let stop_index: FxHashMap<_, _> = (stop_ids.iter().enumerate())
            .map(|(i, &id)| (id, i as u32))
            .collect();
        let mut footpaths_out = vec![Vec::new(); stop_ids.len()];
        let mut footpaths_in = vec![Vec::new(); stop_ids.len()];
        for f in &timetable.footpaths {
            if let (Some(&from), Some(&to)) = (stop_index.get(&f.from), stop_index.get(&f.to)) {
                footpaths_out[from as usize].push((to, f.minutes));
                footpaths_in[to as usize].push((from, f.minutes));
            }
        }
        let transfers = TransferRules::new(&timetable, &stop_ids, &stop_index);
        Self {
            timetable,
            stop_ids,
            stop_index,
            footpaths_out,
            footpaths_in,
            transfers,
            days: Mutex::default(),
        }
    }

    fn day(&self, date: NaiveDate) -> Option<Arc<Day>> {
        let tt = &self.timetable;
        if date < tt.start - Duration::days(1) || date > tt.end + Duration::days(1) {
            return None;
        }
        let mut days = self.days.lock().unwrap();
        if let Some(day) = days.iter().find(|day| day.date() == date) {
            return Some(day.clone());
        }
        // ponytail: built while holding the lock, so queries on other dates wait for it.
        let forward = Network::build(tt, &self.stop_index, date);
        let day = Arc::new(Day {
            forward,
            backward: OnceLock::new(),
        });
        if days.len() == 3 {
            days.remove(0);
        }
        days.push(day.clone());
        Some(day)
    }

    /// Builds the networks of `date` now rather than at its first query. False outside the
    /// timetable period.
    pub fn prepare(&self, date: NaiveDate) -> bool {
        self.day(date).inspect(|day| _ = day.backward()).is_some()
    }

    fn stop(&self, id: i32) -> Option<u32> {
        self.stop_index.get(&id).copied()
    }

    fn footpaths(&self, backward: bool, stop: u32) -> &[(u32, i16)] {
        let footpaths = if backward {
            &self.footpaths_in
        } else {
            &self.footpaths_out
        };
        &footpaths[stop as usize]
    }

    /// Transfer rules are written for the real direction of travel.
    fn transfer_minutes(
        &self,
        net: &Network,
        stop: u32,
        arriving: Trip,
        departing: Trip,
    ) -> Option<i32> {
        let (from, to) = if net.backward {
            (departing, arriving)
        } else {
            (arriving, departing)
        };
        self.transfers
            .minutes(&self.timetable, net.date, stop, from, to)
    }
}

/// RAPTOR rounds: one trip, plus one per transfer.
fn rounds(max_num_explorable_connections: i32) -> usize {
    usize::try_from(max_num_explorable_connections).unwrap_or(0) + 1
}

fn minutes(at: NaiveDateTime, date: NaiveDate) -> i32 {
    (at - date.and_hms_opt(0, 0, 0).unwrap()).num_minutes() as i32
}

/// Finds the journey arriving the earliest at the arrival stop. Among those, it leaves as late
/// as possible, then has the fewest transfers. The departure date must be in the timetable period.
pub fn plan_journey(
    router: &Router,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Option<Route> {
    let (from, to) = (
        router.stop(departure_stop_id)?,
        router.stop(arrival_stop_id)?,
    );
    let day = router.day(departure_at.date())?;
    let rounds = rounds(max_num_explorable_connections);
    let seed = (from, minutes(departure_at, day.date()));
    let arrival = raptor::run(router, &day.forward, &[seed], Some(to), rounds, INF).time(to)?;
    // The latest departure that still arrives then, but not before `departure_at`.
    let backward = raptor::run(
        router,
        day.backward(),
        &[(to, -arrival)],
        Some(from),
        rounds,
        1 - seed.1,
    );
    finish(
        router,
        &day,
        &backward.legs(day.backward(), from),
        departure_at,
        verbose,
    )
}

/// Finds the journey leaving the latest from the departure stop to arrive by `arrival_at`.
/// Among those, it arrives as early as possible, then has the fewest transfers.
pub fn plan_journey_reverse(
    router: &Router,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    arrival_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Option<Route> {
    let (from, to) = (
        router.stop(departure_stop_id)?,
        router.stop(arrival_stop_id)?,
    );
    let day = router.day(arrival_at.date())?;
    let rounds = rounds(max_num_explorable_connections);
    let seed = (to, -minutes(arrival_at, day.date()));
    let departure =
        -raptor::run(router, day.backward(), &[seed], Some(from), rounds, INF).time(from)?;
    // The earliest arrival when leaving then, but not after `arrival_at`.
    let forward = raptor::run(
        router,
        &day.forward,
        &[(from, departure)],
        Some(to),
        rounds,
        1 - seed.1,
    );
    finish(
        router,
        &day,
        &forward.legs(&day.forward, to),
        arrival_at,
        verbose,
    )
}

fn finish(
    router: &Router,
    day: &Day,
    legs: &[Leg],
    at: NaiveDateTime,
    verbose: bool,
) -> Option<Route> {
    let route = (!legs.is_empty()).then(|| Route::new(router, day.date(), legs, at))?;
    if verbose {
        route.print(&router.timetable);
    }
    Some(route)
}

/// Finds the routes to every stop reachable from the departure stop within the time limit.
pub fn find_reachable_stops_within_time_limit(
    router: &Router,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let (Some(from), Some(day)) = (
        router.stop(departure_stop_id),
        router.day(departure_at.date()),
    ) else {
        return Vec::new();
    };
    let start = minutes(departure_at, day.date());
    let limit = start + time_limit.num_minutes() as i32 + 1;
    let rounds = rounds(max_num_explorable_connections);
    let search = raptor::run(router, &day.forward, &[(from, start)], None, rounds, limit);
    reached(router, &day.forward, &search, departure_at, verbose)
}

/// Finds the routes from every stop that reaches the arrival stop within the time limit.
pub fn find_origin_stops_within_time_limit(
    router: &Router,
    arrival_stop_id: i32,
    arrival_at: NaiveDateTime,
    time_limit: Duration,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let (Some(to), Some(day)) = (router.stop(arrival_stop_id), router.day(arrival_at.date()))
    else {
        return Vec::new();
    };
    let end = minutes(arrival_at, day.date());
    let limit = -end + time_limit.num_minutes() as i32 + 1;
    let rounds = rounds(max_num_explorable_connections);
    let search = raptor::run(router, day.backward(), &[(to, -end)], None, rounds, limit);
    reached(router, day.backward(), &search, arrival_at, verbose)
}

/// Routes from a point: one search from the `num_starting_points` stops within walking range
/// (those with the most time left first), each seeded with its walking time. A last route with
/// no duration stands for the point itself.
#[allow(clippy::too_many_arguments)]
pub fn compute_routes_from_origin(
    router: &Router,
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_at: NaiveDateTime,
    time_limit: Duration,
    num_starting_points: usize,
    _num_threads: usize,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let (lat, lon) = (origin_point_latitude, origin_point_longitude);
    routes_around(
        router,
        (lat, lon),
        departure_at,
        time_limit,
        num_starting_points,
        max_num_explorable_connections,
        verbose,
        false,
    )
}

/// Routes to a point: the reverse of [`compute_routes_from_origin`].
#[allow(clippy::too_many_arguments)]
pub fn compute_routes_to_destination(
    router: &Router,
    destination_latitude: f64,
    destination_longitude: f64,
    arrival_at: NaiveDateTime,
    time_limit: Duration,
    num_starting_points: usize,
    _num_threads: usize,
    max_num_explorable_connections: i32,
    verbose: bool,
) -> Vec<Route> {
    let (lat, lon) = (destination_latitude, destination_longitude);
    routes_around(
        router,
        (lat, lon),
        arrival_at,
        time_limit,
        num_starting_points,
        max_num_explorable_connections,
        verbose,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn routes_around(
    router: &Router,
    (lat, lon): (f64, f64),
    at: NaiveDateTime,
    time_limit: Duration,
    num_starting_points: usize,
    max_num_explorable_connections: i32,
    verbose: bool,
    backward: bool,
) -> Vec<Route> {
    // A backward search runs on negated times, from the arrival.
    let (sign, shifted) = if backward {
        (-1, at - time_limit)
    } else {
        (1, at + time_limit)
    };
    let mut routes = match router.day(at.date()) {
        None => Vec::new(),
        Some(day) => {
            let net = if backward {
                day.backward()
            } else {
                &day.forward
            };
            let walk = |stop: &Stop| walking_time_to(lat, lon, stop);
            let walked = |stop: &Stop| {
                if backward {
                    at - walk(stop)
                } else {
                    at + walk(stop)
                }
            };
            let stops = walking_range(router, num_starting_points, |stop| time_limit - walk(stop));
            let seeds: Vec<_> = stops
                .map(|s| {
                    (
                        router.stop_index[&s.id],
                        sign * minutes(walked(s), day.date()),
                    )
                })
                .collect();
            let limit = sign * minutes(shifted, day.date()) + 1;
            let search = raptor::run(
                router,
                net,
                &seeds,
                None,
                rounds(max_num_explorable_connections),
                limit,
            );
            reached(router, net, &search, at, verbose)
        }
    };
    let (easting, northing) = wgs84_to_lv95(lat, lon);
    let (point, nowhere) = (
        Some(Coordinates::new(CoordinateSystem::LV95, easting, northing)),
        Some(Coordinates::default()),
    );
    routes.push(if backward {
        point_route(at, NaiveDateTime::default(), point, nowhere)
    } else {
        point_route(NaiveDateTime::default(), at, nowhere, point)
    });
    routes
}

/// The `count` stops with the most `time_left` (if positive) after walking to or from them.
fn walking_range(
    router: &Router,
    count: usize,
    time_left: impl Fn(&Stop) -> Duration,
) -> impl Iterator<Item = &Stop> {
    let stops = router
        .timetable
        .stops
        .values()
        .filter(|s| s.wgs84.is_some());
    let mut stops: Vec<_> = stops
        .map(|s| (time_left(s), s))
        .filter(|(t, _)| t.num_minutes() > 0)
        .collect();
    stops.sort_by(|(t1, s1), (t2, s2)| t2.cmp(t1).then(s1.id.cmp(&s2.id)));
    stops.into_iter().take(count).map(|(_, s)| s)
}

/// The routes to every stop reached by at least one trip.
fn reached(
    router: &Router,
    net: &Network,
    search: &Search,
    at: NaiveDateTime,
    verbose: bool,
) -> Vec<Route> {
    let routes: Vec<_> = (0..search.stop_count() as u32)
        .filter(|&stop| search.time(stop).is_some())
        .map(|stop| search.legs(net, stop))
        .filter(|legs| legs.iter().any(|leg| matches!(leg, Leg::Ride { .. })))
        .map(|legs| Route::new(router, net.date, &legs, at))
        .collect();
    if verbose {
        log::info!("{} stops reached", routes.len());
    }
    routes
}

fn point_route(
    departure_at: NaiveDateTime,
    arrival_at: NaiveDateTime,
    departure_stop_lv95: Option<Coordinates>,
    arrival_stop_lv95: Option<Coordinates>,
) -> Route {
    let section = RouteSection {
        journey: None,
        departure_stop_id: 0,
        arrival_stop_id: 0,
        departure_at: NaiveDateTime::default(),
        arrival_at: NaiveDateTime::default(),
        departure_stop_lv95,
        arrival_stop_lv95,
    };
    Route {
        departure_at,
        arrival_at,
        sections: vec![section],
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};
    use hrdf_parser::{
        Footpath, Journey, JourneyTransferTime, LineSelector, LineTransferTime, Section, Stop,
        StopTime, Timetable,
    };

    use super::*;

    const A: i32 = 1;
    const B: i32 = 2;
    const C: i32 = 3;
    const D: i32 = 4;

    fn hm(h: i16, m: i16) -> Option<i16> {
        Some(h * 60 + m)
    }

    fn at(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 3, day)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    /// A journey running every day; calls are (stop, arrival, departure).
    fn journey(number: i32, calls: &[(i32, Option<i16>, Option<i16>)]) -> Journey {
        let stops = calls.iter().map(|&(stop_id, arrival, departure)| StopTime {
            stop_id,
            arrival,
            departure,
            can_alight: arrival.is_some(),
            can_board: departure.is_some(),
            platform: None,
        });
        let last = calls.len() as u16 - 1;
        Journey {
            number,
            admin: "000011".into(),
            transport: "B".into(),
            stops: stops.collect(),
            sections: vec![Section {
                first: 0,
                last,
                bit_field: 0,
            }],
            ..Default::default()
        }
    }

    fn router(journeys: Vec<Journey>, footpaths: Vec<Footpath>) -> Router {
        Router::new(timetable(journeys, footpaths))
    }

    fn timetable(journeys: Vec<Journey>, footpaths: Vec<Footpath>) -> Timetable {
        let stop = |id| {
            (
                id,
                Stop {
                    id,
                    name: format!("S{id}"),
                    transfer_allowed: true,
                    ..Default::default()
                },
            )
        };
        Timetable {
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            stops: [A, B, C, D].into_iter().map(stop).collect(),
            journeys,
            footpaths,
            default_transfer_time: (2, 2),
            ..Default::default()
        }
    }

    fn numbers(router: &Router, route: &Route) -> Vec<i32> {
        let journeys = route.sections.iter().filter_map(|s| s.journey);
        journeys
            .map(|(j, ..)| router.timetable.journeys[j].number)
            .collect()
    }

    /// Two buses reach the train at B; the later one avoids waiting.
    fn two_buses_one_train() -> Vec<Journey> {
        vec![
            journey(1, &[(A, None, hm(8, 2)), (B, hm(8, 10), None)]),
            journey(2, &[(A, None, hm(8, 20)), (B, hm(8, 28), None)]),
            journey(3, &[(B, None, hm(8, 35)), (C, hm(9, 0), None)]),
        ]
    }

    #[test]
    fn plan_journey_arrives_earliest_and_leaves_latest() {
        let router = router(two_buses_one_train(), vec![]);
        let route = plan_journey(&router, A, C, at(10, 8, 0), 3, false).unwrap();
        assert_eq!(
            (route.departure_at, route.arrival_at),
            (at(10, 8, 20), at(10, 9, 0))
        );
        assert_eq!(numbers(&router, &route), [2, 3]);
    }

    #[test]
    fn plan_journey_reverse_leaves_latest() {
        let router = router(two_buses_one_train(), vec![]);
        let route = plan_journey_reverse(&router, A, C, at(10, 9, 30), 3, false).unwrap();
        assert_eq!(
            (route.departure_at, route.arrival_at),
            (at(10, 8, 20), at(10, 9, 0))
        );
    }

    #[test]
    fn transfer_time_and_round_limit_are_respected() {
        let mut journeys = two_buses_one_train();
        journeys[1] = journey(2, &[(A, None, hm(8, 20)), (B, hm(8, 34), None)]);
        let router = router(journeys, vec![]);
        // Bus 2 arrives one minute before the train but changing takes two.
        let route = plan_journey(&router, A, C, at(10, 8, 0), 3, false).unwrap();
        assert_eq!(numbers(&router, &route), [1, 3]);
        // No transfer allowed: no route.
        assert!(plan_journey(&router, A, C, at(10, 8, 0), 0, false).is_none());
    }

    #[test]
    fn journey_pair_rule_overrides_the_default() {
        let mut journeys = two_buses_one_train();
        journeys[1] = journey(2, &[(A, None, hm(8, 20)), (B, hm(8, 34), None)]);
        let mut tt = timetable(journeys, vec![]);
        // A guaranteed connection: 0 minutes from bus 2 to the train.
        let rule = JourneyTransferTime {
            stop_id: B,
            from: (2, "000011".into()),
            to: (3, "000011".into()),
            minutes: 0,
            bit_field: 0,
        };
        tt.journey_transfer_times.push(rule);
        let router = Router::new(tt);
        let route = plan_journey(&router, A, C, at(10, 8, 0), 3, false).unwrap();
        assert_eq!(numbers(&router, &route), [2, 3]);
    }

    #[test]
    fn line_rule_overrides_the_default() {
        let mut journeys = two_buses_one_train();
        journeys[1] = journey(2, &[(A, None, hm(8, 20)), (B, hm(8, 34), None)]);
        journeys[1].line = Some(0);
        let mut tt = timetable(journeys, vec![]);
        tt.lines = vec!["7".into()];
        // Line 7 buses connect to any train at B in a minute.
        let line_7 = LineSelector {
            line: Some("7".into()),
            ..Default::default()
        };
        let rule = LineTransferTime {
            stop_id: Some(B),
            from: line_7,
            to: LineSelector::default(),
            minutes: 1,
        };
        tt.line_transfer_times.push(rule);
        let router = Router::new(tt);
        let route = plan_journey(&router, A, C, at(10, 8, 0), 3, false).unwrap();
        assert_eq!(numbers(&router, &route), [2, 3]);
    }

    #[test]
    fn boarding_bans_and_footpaths() {
        let mut banned = journey(1, &[(A, None, hm(8, 0)), (C, hm(8, 30), None)]);
        banned.stops[0].can_board = false;
        let journeys = vec![
            banned,
            journey(2, &[(A, None, hm(8, 10)), (B, hm(8, 20), None)]),
            journey(3, &[(D, None, hm(8, 30)), (C, hm(8, 50), None)]),
        ];
        let router = router(
            journeys,
            vec![Footpath {
                from: B,
                to: D,
                minutes: 5,
            }],
        );
        let route = plan_journey(&router, A, C, at(10, 7, 55), 3, false).unwrap();
        assert_eq!(route.arrival_at, at(10, 8, 50));
        let walk = &route.sections[1];
        assert_eq!(
            (walk.journey, walk.departure_stop_id, walk.arrival_stop_id),
            (None, B, D)
        );
        assert_eq!(
            (walk.departure_at, walk.arrival_at),
            (at(10, 8, 20), at(10, 8, 25))
        );
    }

    #[test]
    fn an_arrival_on_foot_needs_no_transfer_time() {
        // B is reached by trip at 08:27, or on foot from C at 08:29. With 7 minutes to change at
        // B, only the walker catches the 08:30 departure, so the earlier arrival must not win.
        let journeys = vec![
            journey(1, &[(A, None, hm(8, 0)), (B, hm(8, 27), None)]),
            journey(2, &[(A, None, hm(8, 0)), (C, hm(8, 20), None)]),
            journey(3, &[(B, None, hm(8, 30)), (D, hm(8, 40), None)]),
        ];
        let mut tt = timetable(
            journeys,
            vec![Footpath {
                from: C,
                to: B,
                minutes: 9,
            }],
        );
        tt.stops.get_mut(&B).unwrap().transfer_time = Some((7, 7));
        let router = Router::new(tt);
        let route = plan_journey(&router, A, D, at(10, 7, 55), 3, false).unwrap();
        assert_eq!(route.arrival_at, at(10, 8, 40));
        assert_eq!(numbers(&router, &route), [2, 3]);
        let route = plan_journey_reverse(&router, A, D, at(10, 8, 40), 3, false).unwrap();
        assert_eq!(numbers(&router, &route), [2, 3]);
    }

    #[test]
    fn trips_cross_midnight() {
        let router = router(
            vec![journey(1, &[(A, None, hm(23, 50)), (B, hm(24, 30), None)])],
            vec![],
        );
        let route = plan_journey(&router, A, B, at(10, 23, 45), 3, false).unwrap();
        assert_eq!(route.arrival_at, at(11, 0, 30));
        let route = plan_journey_reverse(&router, A, B, at(11, 1, 0), 3, false).unwrap();
        assert_eq!(route.departure_at, at(10, 23, 50));
    }

    #[test]
    fn reachable_stops_respect_the_time_limit() {
        let router = router(two_buses_one_train(), vec![]);
        let reached = |minutes| {
            let routes = find_reachable_stops_within_time_limit(
                &router,
                A,
                at(10, 8, 0),
                Duration::minutes(minutes),
                3,
                false,
            );
            let mut stops: Vec<_> = routes
                .iter()
                .map(|r| r.sections.last().unwrap().arrival_stop_id)
                .collect();
            stops.sort();
            stops
        };
        assert_eq!(reached(30), [B]);
        assert_eq!(reached(60), [B, C]);
        let origins = find_origin_stops_within_time_limit(
            &router,
            C,
            at(10, 9, 0),
            Duration::minutes(45),
            3,
            false,
        );
        let departures: Vec<_> = origins
            .iter()
            .map(|r| (r.sections[0].departure_stop_id, r.departure_at))
            .collect();
        assert!(departures.contains(&(A, at(10, 8, 20))));
    }
}
