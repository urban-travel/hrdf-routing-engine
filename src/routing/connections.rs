use chrono::{Duration, NaiveDate, NaiveDateTime};
use hrdf_parser::{timetable_end_date, DataStorage,  Trip, Model, TransportType};
use rustc_hash::FxHashSet;

use crate::utils::{
    add_1_day, add_minutes_to_date_time, count_days_between_two_dates, create_time,
};

use super::{models::Route, utils::get_routes_to_ignore};

pub fn get_connections(
    data_storage: &DataStorage,
    route: &Route,
    trips_to_ignore: &FxHashSet<i32>,
) -> Vec<Route> {
    next_departures(
        data_storage,
        route.arrival_stop_id(),
        route.arrival_at(),
        Some(get_routes_to_ignore(data_storage, &route)),
        route.last_section().trip_id(),
    )
    .into_iter()
    // A trip is removed if it has already been explored at a lower connection level.
    .filter(|(trip, _)| !trips_to_ignore.contains(&trip.id()))
    .filter_map(|(trip, trip_departure_at)| {
        route.extend(
            data_storage,
            trip.id(),
            trip_departure_at.date(),
            true,
        )
    })
    .collect()
}

pub fn next_departures<'a>(
    data_storage: &'a DataStorage,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    routes_to_ignore: Option<FxHashSet<u64>>,
    previous_trip_id: Option<i32>,
) -> Vec<(&'a  Trip, NaiveDateTime)> {
    fn get_trips(
        data_storage: &DataStorage,
        date: NaiveDate,
        stop_id: i32,
    ) -> (Vec<(& Trip, NaiveDateTime)>, NaiveDateTime) {
        let mut max_departure_at = NaiveDateTime::new(date, create_time(0, 0));

        let trips = get_operating_trips(data_storage, date, stop_id)
            .into_iter()
            .filter(|trip| !trip.is_last_stop(stop_id, true))
            .map(|trip| {
                let trip_departure_at = trip.departure_at_of(stop_id, date);
                if trip_departure_at > max_departure_at {
                    max_departure_at = trip_departure_at;
                }
                (trip, trip_departure_at)
            })
            .collect();
        (trips, max_departure_at)
    }

    let (trips_1, mut max_depearture_at_trips_1_adjusted) =
        get_trips(data_storage, departure_at.date(), departure_stop_id);
    max_depearture_at_trips_1_adjusted = max_depearture_at_trips_1_adjusted
        .checked_add_signed(Duration::hours(-4))
        .unwrap();

    let (trips_2, max_departure_at) = if departure_at > max_depearture_at_trips_1_adjusted {
        // The trips of the next day are also loaded.
        // The maximum departure time is 08:00 the next day.
        let departure_date = add_1_day(departure_at.date());
        let (trips, _) = get_trips(data_storage, departure_date, departure_stop_id);
        let max_departure_at = NaiveDateTime::new(departure_date, create_time(8, 0));

        (trips, max_departure_at)
    } else {
        let max_departure_at = if departure_at.time() < create_time(8, 0) {
            // The maximum departure time is 08:00.
            NaiveDateTime::new(departure_at.date(), create_time(8, 0))
        } else {
            // The maximum departure time is 4 hours later.
            departure_at.checked_add_signed(Duration::hours(4)).unwrap()
        };

        (Vec::new(), max_departure_at)
    };

    let mut trips: Vec<(& Trip, NaiveDateTime)> = [trips_1, trips_2]
        .concat()
        .into_iter()
        .filter(|&(_, trip_departure_at)| {
            //  Trips that depart too early or too late are ignored.
            trip_departure_at >= departure_at && trip_departure_at <= max_departure_at
        })
        .collect();

    //  Trips are sorted by ascending departure time, allowing them to be filtered correctly afterwards.
    trips.sort_by_key(|(_, trip_departure_at)| *trip_departure_at);

    let mut routes_to_ignore = routes_to_ignore.unwrap_or_else(FxHashSet::default);

    trips
        .into_iter()
        .filter(|(trip, _)| {
            let hash = trip.hash_route(departure_stop_id).unwrap();

            if !routes_to_ignore.contains(&hash) {
                // The trip is the first to have this destination (terminus).
                routes_to_ignore.insert(hash);
                true
            } else {
                // The trip has the same destination as another trip, but arrives later.
                // It's ignored.
                false
            }
        })
        .filter(|&(trip, trip_departure_at)| {
            // It is checked that there is enough time to embark on the trip (exchange time).
            previous_trip_id.map_or(true, |id| {
                let exchange_time = get_exchange_time(
                    data_storage,
                    departure_stop_id,
                    id,
                    trip.id(),
                    trip_departure_at,
                );
                add_minutes_to_date_time(departure_at, exchange_time.into()) <= trip_departure_at
            })
        })
        .collect()
}

pub fn get_operating_trips(
    data_storage: &DataStorage,
    date: NaiveDate,
    stop_id: i32,
) -> Vec<& Trip> {
    data_storage
        .bit_fields_by_stop_id()
        .get(&stop_id)
        .map_or(Vec::new(), |bit_fields_1| {
            let bit_fields_2 = data_storage.bit_fields_by_day().get(&date).unwrap();
            let bit_fields: Vec<_> = bit_fields_1.intersection(&bit_fields_2).collect();

            bit_fields
                .into_iter()
                .map(|&bit_field_id| {
                    data_storage
                        .trips_by_stop_id_and_bit_field_id()
                        .get(&(stop_id, bit_field_id))
                        .unwrap()
                })
                .flatten()
                .map(|&trip_id| data_storage.trips().find(trip_id))
                .collect()
        })
}

pub fn get_exchange_time(
    data_storage: &DataStorage,
    stop_id: i32,
    trip_id_1: i32,
    trip_id_2: i32,
    departure_at: NaiveDateTime,
) -> i16 {
    let stop = data_storage.stops().find(stop_id);
    let trip_1 = data_storage.trips().find(trip_id_1);
    let trip_2 = data_storage.trips().find(trip_id_2);

    // Fahrtpaarbezogene Umsteigezeiten /-\  Trip pair-related exchange times.
    if let Some(exchange_time) = exchange_time_trip_pair(
        data_storage,
        stop_id,
        trip_id_1,
        trip_id_2,
        departure_at,
    ) {
        return exchange_time;
    }

    // Linienbezogene Umsteigezeiten an Haltestellen /-\ Line-related exchange times at stops.

    // Verwaltungsbezogene Umsteigezeiten an Haltestellen /-\ Administration-related exchange times at stops.
    if let Some(&id) = data_storage.exchange_times_administration_map().get(&(
        Some(stop_id),
        trip_1.administration().into(),
        trip_2.administration().into(),
    )) {
        return data_storage
            .exchange_times_administration()
            .find(id)
            .duration();
    }

    // Haltestellenbezogene Umsteigezeiten /-\ Stop-related exchange times.
    if let Some(exchange_time) = stop.exchange_time() {
        return exchange_time_at_stop(
            exchange_time,
            trip_1.transport_type(data_storage),
            trip_2.transport_type(data_storage),
        );
    }

    // Linienbezogene Umsteigezeiten (global) /-\ Line-related exchange times (global).

    // Verwaltungsbezogene Umsteigezeiten (global) /-\ Administration-related exchange times (global).
    if let Some(&id) = data_storage.exchange_times_administration_map().get(&(
        None,
        trip_1.administration().into(),
        trip_2.administration().into(),
    )) {
        return data_storage
            .exchange_times_administration()
            .find(id)
            .duration();
    }

    // Standardumsteigezeit /-\ Standard exchange time.
    exchange_time_at_stop(
        data_storage.default_exchange_time(),
        trip_1.transport_type(data_storage),
        trip_2.transport_type(data_storage),
    )
}

fn exchange_time_trip_pair(
    data_storage: &DataStorage,
    stop_id: i32,
    trip_id_1: i32,
    trip_id_2: i32,
    departure_at: NaiveDateTime,
) -> Option<i16> {
    let Some(exchange_times) =
        data_storage
            .exchange_times_trip_map()
            .get(&(stop_id, trip_id_1, trip_id_2))
    else {
        return None;
    };

    // "2 +" because a 2-bit offset is mandatory.
    // "- 1" to obtain an index.
    let index = 2 + count_days_between_two_dates(
        departure_at.date(),
        timetable_end_date(data_storage.timetable_metadata()).unwrap(),
    ) - 1;

    for &id in exchange_times {
        let exchange_time = data_storage.exchange_times_trip().find(id);

        if let Some(bit_field_id) = exchange_time.bit_field_id() {
            let bit_field = data_storage.bit_fields().find(bit_field_id);

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
