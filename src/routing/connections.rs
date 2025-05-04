use chrono::{Duration, NaiveDate, NaiveDateTime};
use hrdf_parser::{DataStorage, Journey, Model, TransportType, timetable_end_date};
use rustc_hash::FxHashSet;

use crate::utils::{
    add_1_day, add_minutes_to_date_time, count_days_between_two_dates, create_time,
};

use super::{models::Route, utils::get_routes_to_ignore};

pub fn get_connections(
    data_storage: &DataStorage,
    route: &Route,
    journeys_to_ignore: &FxHashSet<i32>,
) -> Vec<Route> {
    next_departures(
        data_storage,
        route.arrival_stop_id(),
        route.arrival_at(),
        Some(get_routes_to_ignore(data_storage, route)),
        route.last_section().journey_id(),
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

pub fn next_departures(
    data_storage: &DataStorage,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    routes_to_ignore: Option<FxHashSet<u64>>,
    previous_journey_id: Option<i32>,
) -> Vec<(&Journey, NaiveDateTime)> {
    fn get_journeys(
        data_storage: &DataStorage,
        date: NaiveDate,
        stop_id: i32,
    ) -> (Vec<(&Journey, NaiveDateTime)>, NaiveDateTime) {
        let mut max_departure_at = NaiveDateTime::new(date, create_time(0, 0));

        let journeys = get_operating_journeys(data_storage, date, stop_id)
            .into_iter()
            .filter(|journey| !journey.is_last_stop(stop_id, true))
            .map(|journey| {
                let journey_departure_at = journey.departure_at_of(stop_id, date);
                if journey_departure_at > max_departure_at {
                    max_departure_at = journey_departure_at;
                }
                (journey, journey_departure_at)
            })
            .collect();
        (journeys, max_departure_at)
    }

    let (journeys_1, mut max_depearture_at_journeys_1_adjusted) =
        get_journeys(data_storage, departure_at.date(), departure_stop_id);
    max_depearture_at_journeys_1_adjusted = max_depearture_at_journeys_1_adjusted
        .checked_add_signed(Duration::hours(-4))
        .unwrap();

    let (journeys_2, max_departure_at) = if departure_at > max_depearture_at_journeys_1_adjusted {
        // The journeys of the next day are also loaded.
        // The maximum departure time is 08:00 the next day.
        let departure_date = add_1_day(departure_at.date());
        let (journeys, _) = get_journeys(data_storage, departure_date, departure_stop_id);
        let max_departure_at = NaiveDateTime::new(departure_date, create_time(8, 0));

        (journeys, max_departure_at)
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

    let mut journeys: Vec<(&Journey, NaiveDateTime)> = [journeys_1, journeys_2]
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
            let hash = journey.hash_route(departure_stop_id).unwrap();

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
            let bit_fields_2 = data_storage.bit_fields_by_day().get(&date).unwrap();
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
            journey_1.transport_type(data_storage),
            journey_2.transport_type(data_storage),
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
        journey_1.transport_type(data_storage),
        journey_2.transport_type(data_storage),
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
            .unwrap_or_else(|| panic!("Exchange time journey {:?} not found.", id));

        if let Some(bit_field_id) = exchange_time.bit_field_id() {
            let bit_field = data_storage
                .bit_fields()
                .find(bit_field_id)
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
