mod models;
mod print;
mod storage;

use std::cmp::min;

use hrdf_parser::Hrdf;

use chrono::{Duration, NaiveDateTime, NaiveTime};
use rustc_hash::{FxHashMap, FxHashSet};

pub use storage::RrStorage;

use crate::routing::models::RrRoute;

/// Finds the fastest route from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(
    _hrdf: &Hrdf,
    rr_storage: &RrStorage,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    verbose: bool,
) {
    let departure_stop_index = rr_storage
        .stops()
        .iter()
        .position(|s| s.id() == departure_stop_id)
        .unwrap();
    let arrival_stop_index = rr_storage
        .stops()
        .iter()
        .position(|s| s.id() == arrival_stop_id)
        .unwrap();

    let mut labels = vec![FxHashMap::default()];
    labels[0].insert(departure_stop_index, departure_at.time());

    let mut earliest_arrival_times = FxHashMap::default();

    let mut marked_stops = FxHashSet::default();
    marked_stops.insert(departure_stop_index);

    let mut path = vec![FxHashMap::default()];

    let mut k = 1;
    loop {
        labels.push(FxHashMap::default());
        path.push(FxHashMap::default());

        let round_routes = get_round_routes(rr_storage, &marked_stops);

        if verbose {
            println!("{k} : {}", round_routes.len());
        }

        marked_stops.clear();

        scan_routes(
            departure_at,
            &mut earliest_arrival_times,
            round_routes,
            arrival_stop_index,
            &mut labels,
            k,
            &mut marked_stops,
            &mut path,
        );
        scan_transfers(rr_storage, departure_at, k, &mut marked_stops, &mut labels);

        if marked_stops.is_empty() {
            break;
        }

        k += 1;
    }

    if verbose {
        println!("\nDeparture at: {}\n", departure_at);

        let mut least_trips_label_index = 0;

        for k in 0..labels.len() {
            println!("{:?}", labels[k].get(&arrival_stop_index));

            if labels[k].get(&arrival_stop_index).is_some() && least_trips_label_index == 0 {
                least_trips_label_index = k;
            }
        }
    }

    // let mut dest_stop_index = arrival_stop_index;
    // for k in (0..least_trips_label_index).rev() {
    //     let (trip_index, src_stop_index) = *path[k].get(&dest_stop_index).unwrap();

    //     let stop_1 = data_storage.stops().find(stops[src_stop_index].id());
    //     let stop_2 = data_storage.stops().find(stops[dest_stop_index].id());

    //     println!("{} => {}", stop_1.name(), stop_2.name());

    //     dest_stop_index = src_stop_index;
    // }
}

fn get_round_routes<'a>(
    rr_storage: &'a RrStorage,
    marked_stops: &FxHashSet<usize>,
) -> FxHashMap<usize, (&'a RrRoute, usize)> {
    let mut routes = FxHashMap::default();

    marked_stops.iter().for_each(|&stop_index| {
        let stop = &rr_storage.stops()[stop_index];

        for &route_index in stop.routes() {
            let route = &rr_storage.routes()[route_index];

            let local_stop_index = *route
                .local_stop_index_by_stop_index()
                .get(&stop_index)
                .unwrap();

            // Same route, different stop index.
            let other_route: Option<&(&RrRoute, usize)> = routes.get(&route_index);

            if other_route.is_none() || local_stop_index < (*other_route.unwrap()).1 {
                routes.insert(route_index, (route, local_stop_index));
            }
        }
    });

    routes
}

fn scan_routes(
    origin_departure_at: NaiveDateTime,
    earliest_arrival_times: &mut FxHashMap<usize, NaiveTime>,
    k_routes: FxHashMap<usize, (&RrRoute, usize)>,
    arrival_stop_index: usize,
    labels: &mut Vec<FxHashMap<usize, chrono::NaiveTime>>,
    k: usize,
    marked_stops: &mut FxHashSet<usize>,
    path: &mut Vec<FxHashMap<usize, (usize, usize)>>,
) {
    for (_, (route, stop_local_index)) in k_routes {
        let mut current_trip_index: Option<usize> = None;
        let mut current_trip_boarded_at = 0;

        for stop_i_local_index in stop_local_index..route.stops().len() {
            // Index in stops Vec.
            let stop_i_index = route.stops()[stop_i_local_index];

            if let Some(trip_index) = current_trip_index {
                let arrival_time_at_stop_i =
                    route.arrival_time(trip_index, stop_i_local_index).unwrap();

                // Case: Stop A (23:54), Stop B (00:04), ...
                if arrival_time_at_stop_i < origin_departure_at.time() {
                    break;
                }

                let earliest_arrival_time_stop_i = earliest_arrival_times.get(&stop_i_index);
                let earliest_arrival_time_arrival_stop =
                    earliest_arrival_times.get(&arrival_stop_index);

                let can_label_be_improved = match (
                    earliest_arrival_time_stop_i,
                    earliest_arrival_time_arrival_stop,
                ) {
                    (None, None) => true,
                    (Some(arrival_time_1), None) => arrival_time_at_stop_i < *arrival_time_1,
                    (None, Some(arrival_time_2)) => arrival_time_at_stop_i < *arrival_time_2,
                    (Some(arrival_time_1), Some(arrival_time_2)) => {
                        arrival_time_at_stop_i < min(*arrival_time_1, *arrival_time_2)
                    }
                };

                if can_label_be_improved {
                    labels[k].insert(stop_i_index, arrival_time_at_stop_i);
                    earliest_arrival_times.insert(stop_i_index, arrival_time_at_stop_i);
                    marked_stops.insert(stop_i_index);

                    path[k - 1].insert(stop_i_index, (trip_index, current_trip_boarded_at));
                }
            }

            if stop_i_local_index == route.stops().len() - 1 {
                // It is not possible to board another trip, as the current stop is the terminus.
                continue;
            }

            let previous_arrival_time_at_stop_i = labels[k - 1].get(&stop_i_index);

            let can_catch_earlier_trip = match (previous_arrival_time_at_stop_i, current_trip_index)
            {
                (Some(previous_arrival_time_at_stop_i), Some(trip_index)) => {
                    let departure_time = route.departure_time(trip_index, stop_i_local_index);
                    match departure_time {
                        Some(departure_time) => *previous_arrival_time_at_stop_i <= departure_time,
                        None => false,
                    }
                }
                (Some(_), None) => true,
                _ => false,
            };

            if !can_catch_earlier_trip {
                continue;
            }

            let start_index = if let Some(trip_index) = current_trip_index {
                trip_index
            } else {
                route.trips().len()
            };

            for i in (0..start_index).rev() {
                let departure_time = route.departure_time(i, stop_i_local_index);

                if departure_time.unwrap() < origin_departure_at.time() {
                    // Case: Stop A (23:54), Stop B (00:04), ...
                    continue;
                }

                if departure_time.unwrap() >= *previous_arrival_time_at_stop_i.unwrap() {
                    current_trip_index = Some(i);
                    current_trip_boarded_at = stop_i_index;
                } else {
                    break;
                }
            }
        }
    }
}

fn scan_transfers(
    rr_storage: &RrStorage,
    departure_at: NaiveDateTime,
    k: usize,
    marked_stops: &mut FxHashSet<usize>,
    labels: &mut Vec<FxHashMap<usize, chrono::NaiveTime>>,
) {
    let mut additional_marked_stops = FxHashSet::default();

    for stop_index in marked_stops.iter() {
        let stop = &rr_storage.stops()[*stop_index];

        for i in 0..stop.transfer_count() {
            let transfer = &rr_storage.transfers()[stop.transfer_first_index() + i];

            let arrival_time_1 = labels[k].get(&transfer.other_stop_index());
            let arrival_time_2 =
                *labels[k].get(stop_index).unwrap() + Duration::minutes(transfer.duration() as i64);

            let value = if let Some(arrival_time_1) = arrival_time_1 {
                min(*arrival_time_1, arrival_time_2)
            } else {
                arrival_time_2
            };

            if value < departure_at.time() {
                continue;
            }

            labels[k].insert(transfer.other_stop_index(), value);
            additional_marked_stops.insert(transfer.other_stop_index());
        }
    }

    marked_stops.extend(additional_marked_stops);
}
