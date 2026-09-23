use std::sync::{Arc, RwLock};

use chrono::{Duration, NaiveDate, NaiveDateTime};
use hrdf_parser::{DataStorage, Journey, Model, TransportType, timetable_end_date};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::utils::{
    add_1_day, add_minus_1_day, add_minutes_to_date_time, count_days_between_two_dates, create_time,
};

use super::{models::Route, utils::get_routes_to_ignore};

/// Timetable events and their boundary time: latest departure or earliest arrival.
type CachedJourneys<'a> = (Arc<[(&'a Journey, NaiveDateTime)]>, NaiveDateTime);

/// Maps `(stop_id, service_date)` to `(departures, latest_departure)`.
/// Departures are unsorted `(borrowed Journey, departure_datetime)` pairs.
/// An empty list uses service-date midnight as its latest departure.
/// Entries are shared within one request, before query-specific filtering.
pub(crate) type DepartureCache<'a> = RwLock<FxHashMap<(i32, NaiveDate), CachedJourneys<'a>>>;

/// Maps `(stop_id, service_date)` to `(arrivals, earliest_arrival)`.
/// Like DepartureCache, this stores timetable data before transfer-time filtering.
/// Arrivals are sorted latest first and reused within one reverse search.
#[derive(Default)]
pub(crate) struct ArrivalCache<'a> {
    /// Earliest useful event time, tightened when a better solution is found.
    pub minimum_time: std::cell::Cell<Option<NaiveDateTime>>,
    by_stop_and_date: RwLock<FxHashMap<(i32, NaiveDate), CachedJourneys<'a>>>,
}

/// Merge sorted timetable slices without copying or sorting a full service day
/// for every query. Equal timestamps keep the current day's original precedence.
fn arrivals_in_window<'data: 'slice, 'slice>(
    first: &'slice [(&'data Journey, NaiveDateTime)],
    second: &'slice [(&'data Journey, NaiveDateTime)],
    earliest: NaiveDateTime,
    latest: NaiveDateTime,
) -> impl Iterator<Item = (&'data Journey, NaiveDateTime)> + 'slice {
    let first_start = first.partition_point(|(_, time)| *time > latest);
    let first_end = first.partition_point(|(_, time)| *time >= earliest);
    let second_start = second.partition_point(|(_, time)| *time > latest);
    let second_end = second.partition_point(|(_, time)| *time >= earliest);
    let mut first = first[first_start..first_end].iter().copied().peekable();
    let mut second = second[second_start..second_end].iter().copied().peekable();
    std::iter::from_fn(move || match (first.peek(), second.peek()) {
        (Some((_, a)), Some((_, b))) if a >= b => first.next(),
        (Some(_), Some(_)) => second.next(),
        (Some(_), None) => first.next(),
        (None, Some(_)) => second.next(),
        (None, None) => None,
    })
}

/// Conservative transfer-ready times for reverse searches, indexed only by stop.
/// Footpaths have their own edge times because they can be explored without an interchange.
pub(crate) struct ReverseConnectionTimes {
    best_ready_times: FxHashMap<i32, NaiveDateTime>,
    max_exchange_times: FxHashMap<i32, i16>,
    pub footpaths: FxHashMap<(i32, i32), NaiveDateTime>,
    /// Walking `StopConnection`s indexed by arrival stop (`stop_id_2`), the
    /// reverse of `stop_connections_by_stop_id` (keyed by `stop_id_1` only).
    /// Built once per reverse search.
    pub incoming_stop_connections_by_stop_id: FxHashMap<i32, FxHashSet<i32>>,
}

impl ReverseConnectionTimes {
    pub fn new(data_storage: &DataStorage) -> Self {
        let default = data_storage.default_exchange_time();
        let global_max = data_storage
            .exchange_times_administration()
            .data()
            .values()
            .filter(|entry| entry.stop_id().is_none())
            .map(|entry| entry.duration())
            .fold(default.0.max(default.1), i16::max);
        let mut max_exchange_times: FxHashMap<_, _> = data_storage
            .stops()
            .data()
            .values()
            .map(|stop| {
                (
                    stop.id(),
                    stop.exchange_time()
                        .map_or(global_max, |times| times.0.max(times.1)),
                )
            })
            .collect();
        // Include all journey-pair and administration exceptions. Ignoring their
        // dates here only makes the bound more conservative; it cannot prune more.
        for entry in data_storage.exchange_times_administration().data().values() {
            if let Some(stop_id) = entry.stop_id() {
                max_exchange_times
                    .entry(stop_id)
                    .and_modify(|bound| *bound = (*bound).max(entry.duration()))
                    .or_insert(global_max.max(entry.duration()));
            }
        }
        for entry in data_storage.exchange_times_journey().data().values() {
            max_exchange_times
                .entry(entry.stop_id())
                .and_modify(|bound| *bound = (*bound).max(entry.duration()))
                .or_insert(global_max.max(entry.duration()));
        }
        let incoming_stop_connections_by_stop_id =
            data_storage.stop_connections().entries().into_iter().fold(
                FxHashMap::default(),
                |mut acc: FxHashMap<i32, FxHashSet<i32>>, stop_connection| {
                    acc.entry(stop_connection.stop_id_2())
                        .or_default()
                        .insert(stop_connection.id());
                    acc
                },
            );

        Self {
            best_ready_times: FxHashMap::default(),
            max_exchange_times,
            footpaths: FxHashMap::default(),
            incoming_stop_connections_by_stop_id,
        }
    }

    pub fn can_explore(&mut self, route: &Route) -> bool {
        let stop_id = route.arrival_stop_id();
        let time = route.arrival_at();
        // Compare the candidate's raw time with the previous route's guaranteed
        // cutoff. Comparing only adjusted times could discard a transfer whose
        // actual journey-specific allowance is shorter than the bound.
        if self
            .best_ready_times
            .get(&stop_id)
            .is_some_and(|&best| time <= best)
        {
            return false;
        }

        // With no adjacent journey there is no interchange allowance. Otherwise
        // use an upper bound for every possible journey-specific transfer at this stop.
        let allowance = route
            .last_section()
            .journey_id()
            .map_or(0, |_| self.max_exchange_times[&stop_id]);
        let ready_at = add_minutes_to_date_time(time, -i64::from(allowance));
        self.best_ready_times
            .entry(stop_id)
            .and_modify(|best| *best = (*best).max(ready_at))
            .or_insert(ready_at);
        true
    }
}

pub fn get_connections<'a>(
    data_storage: &'a DataStorage,
    route: &Route,
    journeys_to_ignore: &FxHashSet<i32>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    departure_cache: &DepartureCache<'a>,
) -> Vec<Route> {
    next_departures(
        data_storage,
        route.arrival_stop_id(),
        route.arrival_at(),
        Some(get_routes_to_ignore(data_storage, route, hash_route_cache)),
        route.last_section().journey_id(),
        hash_route_cache,
        departure_cache,
    )
    .into_iter()
    // A journey is removed if it has already been explored at a lower connection level.
    .filter(|(journey, _)| !journeys_to_ignore.contains(&journey.id()))
    .filter_map(|(journey, journey_departure_at)| {
        route.extend(
            data_storage,
            journey.id(),
            journey_departure_at.date(),
            true,
        )
    })
    .collect()
}

pub fn get_connections_reverse<'a>(
    data_storage: &'a DataStorage,
    route: &Route,
    journeys_to_ignore: &FxHashSet<i32>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    arrival_cache: &ArrivalCache<'a>,
) -> Vec<Route> {
    previous_departures(
        data_storage,
        route.arrival_stop_id(),
        route.arrival_at(),
        // hash_route() hashes the forward suffix, the wrong dimension for
        // reverse dedup. journeys_to_ignore already blocks re-using a
        // journey (create_initial_routes_reverse passes None here too).
        None,
        journeys_to_ignore,
        hash_route_cache,
        arrival_cache,
    )
    .into_iter()
    .filter(|&(journey, journey_arrival_at)| {
        // It is checked that there is enough time to embark on the journey (exchange time).
        route.last_section().journey_id().is_none_or(|id| {
            let next_journey = data_storage
                .journeys()
                .find(id)
                .expect("Error: next journey not found");

            // We check if the pair legagy_id is the same because it indicates
            // that it is the same train continuing the journey although they are stored as
            // separated journey in the hrdf format for an unknown reason
            if !has_through_service(
                data_storage,
                route.arrival_at().date(),
                journey.legacy_id(),
                journey.administration(),
                next_journey.legacy_id(),
                next_journey.administration(),
                route.arrival_stop_id(),
            ) {
                let exchange_time = get_exchange_time(
                    data_storage,
                    route.arrival_stop_id(),
                    journey.id(),
                    id,
                    route.arrival_at(),
                );
                add_minutes_to_date_time(journey_arrival_at, exchange_time.into())
                    <= route.arrival_at()
            } else {
                true
            }
        })
    })
    .filter_map(|(journey, journey_arrival_at)| {
        route.extend_reverse(data_storage, journey.id(), journey_arrival_at.date(), false)
    })
    .collect()
}

pub fn previous_departures<'a>(
    data_storage: &'a DataStorage,
    arrival_stop_id: i32,
    arrival_at: NaiveDateTime,
    routes_to_ignore: Option<FxHashSet<u64>>,
    journeys_to_ignore: &FxHashSet<i32>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    arrival_cache: &ArrivalCache<'a>,
) -> Vec<(&'a Journey, NaiveDateTime)> {
    fn get_journeys<'a>(
        data_storage: &'a DataStorage,
        date: NaiveDate,
        stop_id: i32,
        cache: &ArrivalCache<'a>,
    ) -> CachedJourneys<'a> {
        if let Some(value) = cache
            .by_stop_and_date
            .read()
            .unwrap()
            .get(&(stop_id, date))
            .cloned()
        {
            return value;
        }
        let mut min_arrival_at = NaiveDateTime::new(date, create_time(23, 59));

        let mut journeys: Vec<_> = get_operating_journeys(data_storage, date, stop_id)
            .into_iter()
            .filter(|journey| !journey.is_first_stop(stop_id, true).unwrap())
            .filter_map(|journey| {
                let journey_arrival_at = journey.arrival_at_of(stop_id, date).ok()?;
                if journey_arrival_at < min_arrival_at {
                    min_arrival_at = journey_arrival_at;
                }
                Some((journey, journey_arrival_at))
            })
            .collect();
        journeys.sort_by(|(_, a), (_, b)| b.cmp(a));
        let prepared = (Arc::from(journeys), min_arrival_at);
        cache
            .by_stop_and_date
            .write()
            .unwrap()
            .entry((stop_id, date))
            .or_insert(prepared)
            .clone()
    }

    let (journeys_1, mut min_arrival_at_journeys_1_adjusted) = get_journeys(
        data_storage,
        arrival_at.date(),
        arrival_stop_id,
        arrival_cache,
    );
    min_arrival_at_journeys_1_adjusted = min_arrival_at_journeys_1_adjusted
        .checked_add_signed(Duration::hours(4))
        .unwrap();

    let (journeys_2, min_arrival_at) = if arrival_at < min_arrival_at_journeys_1_adjusted {
        // The journeys of the previous day are also loaded.
        // The minimum arrival time is 20:00 the previous day.
        let previous_date = add_minus_1_day(arrival_at.date());
        let (journeys, _) =
            get_journeys(data_storage, previous_date, arrival_stop_id, arrival_cache);
        let min_arrival_at = NaiveDateTime::new(previous_date, create_time(20, 0));

        (journeys, min_arrival_at)
    } else {
        // Always 4 hours earlier: past 20:00 this is always before 20:00
        // too, since 20:00-24:00 is itself only 4 hours wide.
        let min_arrival_at = arrival_at.checked_sub_signed(Duration::hours(4)).unwrap();

        (Arc::from([]), min_arrival_at)
    };

    let min_arrival_at = arrival_cache
        .minimum_time
        .get()
        .map_or(min_arrival_at, |time| time.max(min_arrival_at));
    if min_arrival_at > arrival_at {
        return Vec::new();
    }
    let routes_to_ignore = routes_to_ignore.unwrap_or_default();
    arrivals_in_window(&journeys_1, &journeys_2, min_arrival_at, arrival_at)
        .filter(|(journey, _)| !journeys_to_ignore.contains(&journey.id()))
        .filter(|(journey, _)| {
            if routes_to_ignore.is_empty() {
                return true;
            }
            let hash = hash_route_cache
                .entry((journey.id(), arrival_stop_id))
                .or_insert_with(|| journey.hash_route(arrival_stop_id))
                .unwrap();
            !routes_to_ignore.contains(&hash)
        })
        .collect()
}

pub fn next_departures<'a>(
    data_storage: &'a DataStorage,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    routes_to_ignore: Option<FxHashSet<u64>>,
    previous_journey_id: Option<i32>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    departure_cache: &DepartureCache<'a>,
) -> Vec<(&'a Journey, NaiveDateTime)> {
    fn get_journeys<'a>(
        data_storage: &'a DataStorage,
        date: NaiveDate,
        stop_id: i32,
        cache: &DepartureCache<'a>,
    ) -> CachedJourneys<'a> {
        if let Some(value) = cache.read().unwrap().get(&(stop_id, date)).cloned() {
            return value;
        }

        let mut max_departure_at = NaiveDateTime::new(date, create_time(0, 0));

        let journeys: Vec<_> = get_operating_journeys(data_storage, date, stop_id)
            .into_iter()
            .filter(|journey| !journey.is_last_stop(stop_id, true).unwrap())
            .filter_map(|journey| {
                let journey_departure_at = journey.departure_at_of(stop_id, date).ok()?;
                if journey_departure_at > max_departure_at {
                    max_departure_at = journey_departure_at;
                }
                Some((journey, journey_departure_at))
            })
            .collect();

        // Allow duplicate work on concurrent misses to keep preparation outside the lock.
        let prepared = (Arc::from(journeys), max_departure_at);
        cache
            .write()
            .unwrap()
            .entry((stop_id, date))
            .or_insert(prepared)
            .clone()
    }

    let (journeys_1, mut max_depearture_at_journeys_1_adjusted) = get_journeys(
        data_storage,
        departure_at.date(),
        departure_stop_id,
        departure_cache,
    );
    max_depearture_at_journeys_1_adjusted = max_depearture_at_journeys_1_adjusted
        .checked_add_signed(Duration::hours(-4))
        .unwrap();

    let (journeys_2, max_departure_at) = if departure_at > max_depearture_at_journeys_1_adjusted {
        // The journeys of the next day are also loaded.
        // The maximum departure time is 08:00 the next day.
        let departure_date = add_1_day(departure_at.date());
        let (journeys, _) = get_journeys(
            data_storage,
            departure_date,
            departure_stop_id,
            departure_cache,
        );
        let max_departure_at = NaiveDateTime::new(departure_date, create_time(8, 0));

        (journeys, max_departure_at)
    } else {
        let max_departure_at = if departure_at.time() < create_time(8, 0) {
            // The maximum departure time is at least 08:00 (but never less than 4 hours
            // from departure_at, otherwise queries close to 08:00 get an unreasonably
            // narrow window).
            NaiveDateTime::new(departure_at.date(), create_time(8, 0))
                .max(departure_at.checked_add_signed(Duration::hours(4)).unwrap())
        } else {
            // The maximum departure time is 4 hours later.
            departure_at.checked_add_signed(Duration::hours(4)).unwrap()
        };

        (Arc::from([]), max_departure_at)
    };

    let mut journeys: Vec<(&Journey, NaiveDateTime)> = [&*journeys_1, &*journeys_2]
        .concat()
        .into_iter()
        .filter(|&(_, journey_departure_at)| {
            // Journeys that depart too early or too late are ignored.
            journey_departure_at >= departure_at && journey_departure_at <= max_departure_at
        })
        .collect();

    // Journeys are sorted by ascending departure time, allowing them to be filtered correctly afterwards.
    journeys.sort_by_key(|(_, journey_departure_at)| *journey_departure_at);

    let mut routes_to_ignore = routes_to_ignore.unwrap_or_default();

    journeys
        .into_iter()
        .filter(|(journey, _)| {
            // `hash_route` only depends on the (journey, stop) pair, never on the query
            // date or the rest of the route, so its results are memoized across the
            // whole search.
            let hash = hash_route_cache
                .entry((journey.id(), departure_stop_id))
                .or_insert_with(|| journey.hash_route(departure_stop_id))
                .unwrap();

            if !routes_to_ignore.contains(&hash) {
                // The journey is the first to have this destination (terminus).
                routes_to_ignore.insert(hash);
                true
            } else {
                // The journey has the same destination as another journey, but arrives later.
                // It's ignored.
                false
            }
        })
        .filter(|&(journey, journey_departure_at)| {
            // It is checked that there is enough time to embark on the journey (exchange time).
            previous_journey_id.is_none_or(|id| {
                let previous_journey = data_storage
                    .journeys()
                    .find(id)
                    .expect("Error: previous journey not found");

                // We check if the pair legagy_id is the same because it indicates
                // that it is the same train continuing the journey although they are stored as
                // separated journey in the hrdf format for an unknown reason
                if !has_through_service(
                    data_storage,
                    departure_at.date(),
                    previous_journey.legacy_id(),
                    previous_journey.administration(),
                    journey.legacy_id(),
                    journey.administration(),
                    departure_stop_id,
                ) {
                    let exchange_time = get_exchange_time(
                        data_storage,
                        departure_stop_id,
                        id,
                        journey.id(),
                        journey_departure_at,
                    );
                    add_minutes_to_date_time(departure_at, exchange_time.into())
                        <= journey_departure_at
                } else {
                    true
                }
            })
        })
        .collect()
}

pub fn get_operating_journeys(
    data_storage: &DataStorage,
    date: NaiveDate,
    stop_id: i32,
) -> Vec<&Journey> {
    data_storage
        .bit_fields_by_stop_id()
        .get(&stop_id)
        .map_or(Vec::new(), |bit_fields_1| {
            let Some(bit_fields_2) = data_storage.bit_fields_by_day().get(&date) else {
                // date is outside the loaded timetable: no services to
                // report, not a panic.
                return Vec::new();
            };
            let bit_fields: Vec<_> = bit_fields_1.intersection(bit_fields_2).collect();

            bit_fields
                .into_iter()
                .flat_map(|&bit_field_id| {
                    data_storage
                        .journeys_by_stop_id_and_bit_field_id()
                        .get(&(stop_id, bit_field_id))
                        .unwrap()
                })
                .map(|&journey_id| {
                    data_storage
                        .journeys()
                        .find(journey_id)
                        .unwrap_or_else(|| panic!("Journey {:?} not found.", journey_id))
                })
                .collect()
        })
}

fn has_through_service(
    data_storage: &DataStorage,
    date: NaiveDate,
    journey_1_legacy_id: i32,
    journey_1_admin: &str,
    journey_2_legacy_id: i32,
    journey_2_admin: &str,
    stop_id: i32,
) -> bool {
    let through_service_bitfield = data_storage
        .bit_field_id_for_through_service_by_journey_id_stop_id()
        .get(&(
            (journey_1_legacy_id, journey_1_admin.to_string()),
            (journey_2_legacy_id, journey_2_admin.to_string()),
            stop_id,
        ));
    through_service_bitfield.is_some_and(|bf| {
        let bit_fields_2 = data_storage.bit_fields_by_day().get(&date).unwrap();
        bit_fields_2.contains(bf)
    })
}

pub fn get_exchange_time(
    data_storage: &DataStorage,
    stop_id: i32,
    journey_id_1: i32,
    journey_id_2: i32,
    departure_at: NaiveDateTime,
) -> i16 {
    let stop = data_storage
        .stops()
        .find(stop_id)
        .unwrap_or_else(|| panic!("Stop {:?} not found.", stop_id));
    let journey_1 = data_storage
        .journeys()
        .find(journey_id_1)
        .unwrap_or_else(|| panic!("Journey {:?} not found.", journey_id_1));
    let journey_2 = data_storage
        .journeys()
        .find(journey_id_2)
        .unwrap_or_else(|| panic!("Journey {:?} not found.", journey_id_2));

    // Fahrtpaarbezogene Umsteigezeiten /-\ Journey pair-related exchange times.
    if let Some(exchange_time) = exchange_time_journey_pair(
        data_storage,
        stop_id,
        journey_1.legacy_id(),
        journey_1.administration(),
        journey_2.legacy_id(),
        journey_2.administration(),
        departure_at,
    ) {
        return exchange_time;
    }

    // Linienbezogene Umsteigezeiten an Haltestellen /-\ Line-related exchange times at stops.

    // Verwaltungsbezogene Umsteigezeiten an Haltestellen /-\ Administration-related exchange times at stops.
    if let Some(&id) = data_storage.exchange_times_administration_map().get(&(
        Some(stop_id),
        journey_1.administration().into(),
        journey_2.administration().into(),
    )) {
        return data_storage
            .exchange_times_administration()
            .find(id)
            .unwrap_or_else(|| panic!("Exchange time administration {:?} not found.", id))
            .duration();
    }

    // Haltestellenbezogene Umsteigezeiten /-\ Stop-related exchange times.
    if let Some(exchange_time) = stop.exchange_time() {
        return exchange_time_at_stop(
            exchange_time,
            journey_1
                .transport_type(data_storage)
                .unwrap_or_else(|_| panic!("Error: {journey_1:?} does not have a TransportType.")),
            journey_2
                .transport_type(data_storage)
                .unwrap_or_else(|_| panic!("Error: {journey_2:?} does not have a TransportType.")),
        );
    }

    // Linienbezogene Umsteigezeiten (global) /-\ Line-related exchange times (global).

    // Verwaltungsbezogene Umsteigezeiten (global) /-\ Administration-related exchange times (global).
    if let Some(&id) = data_storage.exchange_times_administration_map().get(&(
        None,
        journey_1.administration().into(),
        journey_2.administration().into(),
    )) {
        return data_storage
            .exchange_times_administration()
            .find(id)
            .unwrap_or_else(|| panic!("Exchange time administration {:?} not found.", id))
            .duration();
    }

    // Standardumsteigezeit /-\ Standard exchange time.
    exchange_time_at_stop(
        data_storage.default_exchange_time(),
        journey_1
            .transport_type(data_storage)
            .unwrap_or_else(|_| panic!("Error: {journey_1:?} does not have a TransportType.")),
        journey_2
            .transport_type(data_storage)
            .unwrap_or_else(|_| panic!("Error: {journey_2:?} does not have a TransportType.")),
    )
}

fn exchange_time_journey_pair(
    data_storage: &DataStorage,
    stop_id: i32,
    journey_legacy_id_1: i32,
    administration_1: &str,
    journey_legacy_id_2: i32,
    administration_2: &str,
    departure_at: NaiveDateTime,
) -> Option<i16> {
    let exchange_times = data_storage.exchange_times_journey_map().get(&(
        stop_id,
        (journey_legacy_id_1, administration_1.to_string()),
        (journey_legacy_id_2, administration_2.to_string()),
    ))?;

    // "2 +" because a 2-bit offset is mandatory.
    // "- 1" to obtain an index.
    let index = 2 + count_days_between_two_dates(
        departure_at.date(),
        timetable_end_date(data_storage.timetable_metadata()).unwrap(),
    ) - 1;

    for &id in exchange_times {
        let exchange_time = data_storage
            .exchange_times_journey()
            .find(id)
            // .expect(format!("Exchange times journey {id} not found").as_str());
            .unwrap_or_else(|| panic!("Exchange time journey {:?} not found.", id));

        if let Some(bit_field_id) = exchange_time.bit_field_id() {
            let bit_field = data_storage
                .bit_fields()
                .find(bit_field_id)
                // .expect(format!("Bit field {bit_field_id} not found").as_str());
                .unwrap_or_else(|| panic!("Bitfield {:?} not found.", bit_field_id));

            if bit_field.bits()[index] == 1 {
                return Some(exchange_time.duration());
            }
        } else {
            return Some(exchange_time.duration());
        }
    }

    None
}

fn exchange_time_at_stop(
    exchange_time: (i16, i16),
    transport_type_1: &TransportType,
    transport_type_2: &TransportType,
) -> i16 {
    if transport_type_1.designation() == "IC" && transport_type_2.designation() == "IC" {
        exchange_time.0
    } else {
        exchange_time.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{routing::models::RouteSection, utils::create_date_time};

    fn times() -> ReverseConnectionTimes {
        ReverseConnectionTimes {
            best_ready_times: FxHashMap::default(),
            max_exchange_times: [(8503000, 7)].into_iter().collect(),
            footpaths: FxHashMap::default(),
            incoming_stop_connections_by_stop_id: FxHashMap::default(),
        }
    }

    fn at_hb(journey_id: Option<i32>, minute: u32) -> Route {
        Route::new(
            vec![RouteSection::new(
                journey_id,
                8591368,
                8503000,
                create_date_time(2025, 11, 25, 9, minute),
                journey_id.is_none().then_some(7),
            )],
            FxHashSet::default(),
        )
    }

    #[test]
    fn reverse_pruning_preserves_a_shorter_transfer_allowance() {
        let mut cache = times();
        assert!(cache.can_explore(&at_hb(Some(1), 31)));
        // 09:31 with up to seven minutes of interchange only guarantees 09:24.
        // A second vehicle at 09:29 might require less interchange time.
        assert!(cache.can_explore(&at_hb(Some(2), 29)));
        assert!(cache.can_explore(&at_hb(None, 29)));
    }

    #[test]
    fn reverse_pruning_uses_the_guaranteed_cutoff() {
        let mut cache = times();
        assert!(cache.can_explore(&at_hb(Some(1), 31)));
        assert!(cache.can_explore(&at_hb(Some(2), 25)));
        assert!(!cache.can_explore(&at_hb(Some(3), 24)));
        assert!(!cache.can_explore(&at_hb(None, 23)));
    }

    #[test]
    fn arrival_window_merges_days_and_preserves_inclusive_boundaries_and_ties() {
        let journeys: Vec<_> = (1..=6)
            .map(|id| Journey::new(id, id, "test".into()))
            .collect();
        let at = |hour, minute| create_date_time(2025, 11, 25, hour, minute);
        let current = [
            (&journeys[0], at(9, 30)),
            (&journeys[1], at(9, 20)),
            (&journeys[2], at(8, 0)),
        ];
        let previous = [
            (&journeys[3], at(9, 25)),
            (&journeys[4], at(9, 20)),
            (&journeys[5], at(7, 0)),
        ];
        let ids: Vec<_> = arrivals_in_window(&current, &previous, at(8, 0), at(9, 25))
            .map(|(j, _)| j.id())
            .collect();
        assert_eq!(ids, vec![4, 2, 5, 3]);
        assert_eq!(
            arrivals_in_window(&[], &previous, at(9, 20), at(9, 20)).count(),
            1
        );
    }
}
