mod models;
mod print;

use std::{cmp::min, time::Instant};

use hrdf_parser::{DataStorage, Hrdf};

use chrono::{Duration, NaiveDateTime};
use models::Journey;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::routing::models::{RrRoute, RrStop, RrStopTime, RrTransfer};

/// Finds the fastest route from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    _verbose: bool,
) -> Option<Journey> {
    let data_storage = hrdf.data_storage();
    let (routes, stop_times, route_stops) = get_routes(data_storage);
    let (stops, stop_routes, transfers) = get_stops(data_storage, &routes, &route_stops);
    let route_stops = fix_route_stops(route_stops, &stops);

    let start_time = Instant::now();

    let departure_stop_index = stops
        .iter()
        .position(|s| s.id() == departure_stop_id)
        .unwrap();
    let arrival_stop_index = stops
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

        let mut k_routes = FxHashMap::default();

        for stop_index in &marked_stops {
            let stop = &stops[*stop_index];

            for route_offset in 0..stop.route_count() {
                let route_index = stop_routes[stop.route_first_index() + route_offset];
                let route = &routes[route_index];

                let mut local_stop_index = 0;
                for i in 0..route.stop_count() {
                    if route_stops[route.stop_first_index() + i] == *stop_index {
                        local_stop_index = i;
                        break;
                    }
                }

                match k_routes.get(&route_index) {
                    Some((_, other_stop_local_index))
                        if local_stop_index > *other_stop_local_index => {}
                    _ => {
                        k_routes.insert(route_index, (route, local_stop_index));
                    }
                }
            }
        }

        println!("{k} : {}", k_routes.len());
        marked_stops.clear();

        for (_, (route, stop_local_index)) in k_routes {
            let mut current_trip_index: Option<usize> = None;
            let mut current_trip_boarded_at = 0;

            for stop_i_local_index in stop_local_index..route.stop_count() {
                // Index in stops Vec.
                let stop_i_index = route_stops[route.stop_first_index() + stop_i_local_index];

                if let Some(current_trip_index) = current_trip_index {
                    let index = current_trip_index + stop_i_local_index;
                    let arrival_time_at_stop_i = stop_times[index].arrival_time().unwrap();

                    // Case: Stop A (23:54), Stop B (00:04), ...
                    if arrival_time_at_stop_i < departure_at.time() {
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

                        path[k - 1]
                            .insert(stop_i_index, (current_trip_index, current_trip_boarded_at));
                    }
                }

                let previous_arrival_time_at_stop_i = labels[k - 1].get(&stop_i_index);

                let can_catch_earlier_trip =
                    match (previous_arrival_time_at_stop_i, current_trip_index) {
                        (Some(previous_arrival_time_at_stop_i), Some(current_trip_index)) => {
                            let index = current_trip_index + stop_i_local_index;
                            match stop_times[index].departure_time() {
                                Some(departure_time) => {
                                    *previous_arrival_time_at_stop_i <= departure_time
                                }
                                None => false,
                            }
                        }
                        (Some(_), None) => true,
                        _ => false,
                    };

                if can_catch_earlier_trip {
                    if let Some(mut i) = current_trip_index {
                        // WARNING: It will probably crash around 0.
                        i += stop_i_local_index;
                        i -= route.stop_count();

                        while i >= route.stop_time_first_index() {
                            if let Some(departure_time) = stop_times[i].departure_time() {
                                if departure_time > *previous_arrival_time_at_stop_i.unwrap() {
                                    let index = i - stop_i_local_index;
                                    current_trip_index = Some(index);
                                    current_trip_boarded_at = stop_i_index;
                                } else {
                                    break;
                                }
                            }

                            i -= route.stop_count();
                        }
                    } else {
                        let mut i = stop_i_local_index;
                        while i < route.stop_time_count() {
                            if let Some(departure_time) =
                                stop_times[route.stop_time_first_index() + i].departure_time()
                            {
                                if departure_time >= *previous_arrival_time_at_stop_i.unwrap() {
                                    let index =
                                        route.stop_time_first_index() + i - stop_i_local_index;
                                    current_trip_index = Some(index);
                                    current_trip_boarded_at = stop_i_index;
                                    break;
                                }
                            }

                            i += route.stop_count();
                        }
                    }
                }
            }
        }

        let mut additional_marked_stops = FxHashSet::default();

        for stop_index in &marked_stops {
            let stop = &stops[*stop_index];

            for i in 0..stop.transfer_count() {
                let transfer = &transfers[stop.transfer_first_index() + i];

                let arrival_time_1 = labels[k].get(&transfer.other_stop_index());
                let arrival_time_2 = *labels[k].get(stop_index).unwrap()
                    + Duration::minutes(transfer.duration() as i64);

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

        if marked_stops.is_empty() {
            break;
        }

        k += 1;
    }

    println!("\nExecution time: {:.2?}", start_time.elapsed());
    println!("Departure at: {}\n", departure_at);

    let mut least_trips_label_index = 0;

    for k in 0..labels.len() {
        println!("{:?}", labels[k].get(&arrival_stop_index));

        if labels[k].get(&arrival_stop_index).is_some() && least_trips_label_index == 0 {
            least_trips_label_index = k;
        }
    }

    // println!();

    // let mut dest_stop_index = arrival_stop_index;
    // for k in (0..least_trips_label_index).rev() {
    //     let (trip_index, src_stop_index) = *path[k].get(&dest_stop_index).unwrap();

    //     let stop_1 = data_storage.stops().find(stops[src_stop_index].id());
    //     let stop_2 = data_storage.stops().find(stops[dest_stop_index].id());

    //     println!("{} => {}", stop_1.name(), stop_2.name());

    //     dest_stop_index = src_stop_index;
    // }

    None
}

fn get_routes(data_storage: &DataStorage) -> (Vec<RrRoute>, Vec<RrStopTime>, Vec<i32>) {
    let mut tmp_routes = FxHashMap::default();

    for trip in data_storage.trips().entries() {
        let route_id = trip.hash_route().unwrap();

        if !tmp_routes.contains_key(&route_id) {
            tmp_routes.insert(route_id, Vec::new());
        }

        tmp_routes.get_mut(&route_id).unwrap().push(trip);
    }

    let mut routes = Vec::new();
    let mut stop_times = Vec::new();
    let mut route_stops = Vec::new();

    for mut trips in tmp_routes.into_values() {
        trips.sort_by(|a, b| {
            let a = a.route().first().unwrap().departure_time();
            let b = b.route().first().unwrap().departure_time();
            a.cmp(b)
        });

        let stop_time_first_index = stop_times.len();

        for trip in &trips {
            for route_entry in trip.route() {
                stop_times.push(RrStopTime::new(
                    *route_entry.arrival_time(),
                    *route_entry.departure_time(),
                ));
            }
        }

        let stop_first_index = route_stops.len();

        for route_entry in trips.first().unwrap().route() {
            route_stops.push(route_entry.stop_id());
        }

        routes.push(RrRoute::new(
            stop_time_first_index,
            stop_times.len() - stop_time_first_index,
            stop_first_index,
            trips.first().unwrap().route().len(),
        ));
    }

    (routes, stop_times, route_stops)
}

fn get_stops(
    data_storage: &DataStorage,
    routes: &Vec<RrRoute>,
    route_stops: &Vec<i32>,
) -> (Vec<RrStop>, Vec<usize>, Vec<RrTransfer>) {
    let mut tmp_stops = FxHashMap::default();

    for (i, route) in routes.iter().enumerate() {
        for j in 0..route.stop_count() {
            let stop_id = route_stops[route.stop_first_index() + j];

            if !tmp_stops.contains_key(&stop_id) {
                tmp_stops.insert(stop_id, Vec::new());
            }

            tmp_stops.get_mut(&stop_id).unwrap().push(i);
        }
    }

    let mut stops = Vec::new();
    let mut stop_routes = Vec::new();
    let mut transfers = Vec::new();

    for (stop_id, routes) in tmp_stops {
        let route_first_index = stop_routes.len();
        let route_count = routes.len();

        for index in routes {
            stop_routes.push(index);
        }

        stops.push(RrStop::new(stop_id, route_first_index, route_count));
    }

    for i in 0..stops.len() {
        let transfer_first_index = transfers.len();

        let stop_connections = data_storage
            .stop_connections_by_stop_id()
            .get(&stops[i].id());

        if stop_connections.is_none() {
            continue;
        }

        for stop_connection_id in stop_connections.unwrap() {
            let stop_connection = data_storage.stop_connections().find(*stop_connection_id);
            let other_stop_index = stops
                .iter()
                .position(|s| s.id() == stop_connection.stop_id_2());

            if let Some(index) = other_stop_index {
                transfers.push(RrTransfer::new(index, stop_connection.duration()));
            }
        }

        stops[i].set_transfer_first_index(transfer_first_index);
        stops[i].set_transfer_count(transfers.len() - transfer_first_index);
    }

    (stops, stop_routes, transfers)
}

fn fix_route_stops(route_stops: Vec<i32>, stops: &Vec<RrStop>) -> Vec<usize> {
    route_stops
        .iter()
        .map(|stop_id| stops.iter().position(|s| s.id() == *stop_id).unwrap())
        .collect()
}
