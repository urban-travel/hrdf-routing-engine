mod models;
mod print;

use std::{
    cmp::min,
    time::{Duration, Instant},
};

use hrdf_parser::{DataStorage, Hrdf, Model};

use chrono::NaiveDateTime;
use models::Journey;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{routing::models::RrRoute, utils::create_time};

/// Finds the fastest route from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(
    hrdf: &Hrdf,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: NaiveDateTime,
    verbose: bool,
) -> Option<Journey> {
    let data_storage = hrdf.data_storage();
    let routes = get_routes(data_storage);
    let routes_by_stop_id = create_routes_by_stop_id(data_storage, &routes);

    let start_time = Instant::now();

    let mut labels = vec![FxHashMap::default()];
    labels[0].insert(departure_stop_id, departure_at.time());

    let mut earliest_known_arrival_time_by_stop_id = FxHashMap::default();

    let mut marked_stops = FxHashSet::default();
    marked_stops.insert(departure_stop_id);

    let mut backtracking = vec![FxHashMap::default()];
    let mut backtracking_foot_paths = vec![FxHashMap::default()];

    let mut k = 1;
    let mut elapsed = Duration::ZERO;
    loop {
        backtracking.push(FxHashMap::default());
        backtracking_foot_paths.push(FxHashMap::default());

        labels.push(FxHashMap::default());
        let mut routes_k = FxHashMap::default();

        for stop_id in marked_stops.iter() {
            if routes_by_stop_id.get(&stop_id).is_none() {
                continue;
            }

            for route in routes_by_stop_id.get(&stop_id).unwrap() {
                routes_k.get_mut(&route.id());

                if let Some((_, value)) = routes_k.get_mut(&route.id()) {
                    let i = route.stops().iter().position(|x| *x == *stop_id);
                    let j = route.stops().iter().position(|x| *x == *value);

                    if i < j {
                        *value = *stop_id;
                    }
                } else {
                    routes_k.insert(route.id(), (route, *stop_id));
                }
            }
        }

        println!("{k} : {}", routes_k.len());
        marked_stops.clear();


        for (_, (route, stop_id)) in routes_k {
            let mut current_trip_id = None;
            let mut boarded_stop = 0;

            let stops = route.stops().iter().skip_while(|s| **s != stop_id);

            for &stop_id_i in stops {
                let start_time = Instant::now();
                if let Some(trip_id) = current_trip_id {
                    let arrival_time = *route.arrival_time(trip_id, stop_id_i).unwrap();

                    // Case: Stop A (23:54), Stop B (00:04), ...
                    if arrival_time < departure_at.time() {
                        break;
                    }

                    let a = earliest_known_arrival_time_by_stop_id.get(&stop_id_i);
                    let b = earliest_known_arrival_time_by_stop_id.get(&arrival_stop_id);
                    let c = a.into_iter().chain(b).min();

                    if c.map_or(true, |t| arrival_time < *t) {
                        labels.last_mut().unwrap().insert(stop_id_i, arrival_time);
                        earliest_known_arrival_time_by_stop_id.insert(stop_id_i, arrival_time);

                        if data_storage
                            .stops()
                            .find(stop_id_i)
                            .can_be_used_as_exchange_point()
                        {
                            marked_stops.insert(stop_id_i);
                        }

                        backtracking[k - 1]
                            .insert(stop_id_i, (current_trip_id.unwrap(), boarded_stop));
                    }
                }
                elapsed += start_time.elapsed();

                let previous_best_arrival_time = labels[k - 1].get(&stop_id_i);

                let can_catch_earlier_trip =
                    previous_best_arrival_time.map_or(false, |arrival_time| {
                        current_trip_id.map_or(true, |trip_id| {
                            let trip_departure_time = route.departure_time(trip_id, stop_id_i);
                            if let Some(trip_departure_time) = trip_departure_time {
                                *arrival_time <= *trip_departure_time
                            } else {
                                false
                            }
                        })
                    });
                if can_catch_earlier_trip {
                    let previous_best_arrival_time = previous_best_arrival_time.unwrap();

                    current_trip_id = route
                        .trips()
                        .iter()
                        .find(|&&trip_id| {
                            let departure_time = route.departure_time(trip_id, stop_id_i);
                            if let Some(departure_time) = departure_time {
                                *departure_time >= *previous_best_arrival_time
                            } else {
                                false
                            }
                        })
                        .cloned();
                    if current_trip_id.is_some() {
                        boarded_stop = stop_id_i;

                        if *route
                            .departure_time(current_trip_id.unwrap(), stop_id_i)
                            .unwrap()
                            < create_time(5, 59)
                        {
                            println!(
                                "{} {} {}",
                                stop_id_i,
                                *route
                                    .departure_time(current_trip_id.unwrap(), stop_id_i)
                                    .unwrap(),
                                previous_best_arrival_time
                            );
                        }
                    }
                }
            }
        }

        let mut new_marked_stops = Vec::new();

        for marked_stop in &marked_stops {
            if let Some(stop_connections) =
                data_storage.stop_connections_by_stop_id().get(&marked_stop)
            {
                for stop_connection_id in stop_connections {
                    let stop_connection = data_storage.stop_connections().find(*stop_connection_id);

                    let arrival_time_stop_1 = labels[k]
                        .get(&stop_connection.stop_id_1())
                        .cloned()
                        .unwrap();
                    let arrival_time_stop_2 = labels[k].get(&stop_connection.stop_id_2()).cloned();

                    if let Some(arrival_time_stop_2) = arrival_time_stop_2 {
                        let a = min(
                            arrival_time_stop_2,
                            arrival_time_stop_1
                                + chrono::Duration::minutes(stop_connection.duration().into()),
                        );
                        if a < departure_at.time() {
                            continue;
                        }
                        labels[k].insert(stop_connection.stop_id_2(), a);
                    } else {
                        let a = arrival_time_stop_1
                            + chrono::Duration::minutes(stop_connection.duration().into());
                        if a < departure_at.time() {
                            continue;
                        }
                        labels[k].insert(stop_connection.stop_id_2(), a);
                    }

                    if let Some(stop) = data_storage
                        .stops()
                        .data()
                        .get(&stop_connection.stop_id_2())
                    {
                        backtracking_foot_paths[k - 1]
                            .insert(stop_connection.stop_id_2(), stop_connection.stop_id_1());
                        if stop.can_be_used_as_exchange_point() {
                            new_marked_stops.push(stop_connection.stop_id_2());
                        }
                    }
                }
            }
        }

        for marked_stop in new_marked_stops {
            marked_stops.insert(marked_stop);
        }

        if marked_stops.is_empty() {
            break;
        }

        k += 1;
    }

    println!("\n{:.2?}", elapsed);
    println!("{:.2?}", start_time.elapsed());
    println!("Departure at: {}", departure_at);
    println!("");
    let mut j = 0;
    for i in 0..labels.len() {
        println!("{:?}", labels[i].get(&arrival_stop_id));

        if labels[i].get(&arrival_stop_id).is_some() && j == 0 {
            j = i;
        }
    }

    println!();

    let mut next_stop_id = arrival_stop_id;
    let mut i: isize = (j - 1) as isize;
    while i >= 0 {
        let xy = backtracking_foot_paths[i as usize].get(&next_stop_id);
        if let Some(xy) = xy {
            let stop_id = xy;
            let stop_1 = data_storage.stops().find(*stop_id);
            let stop_2 = data_storage.stops().find(next_stop_id);
            println!("{} => {}", stop_1.name(), stop_2.name());
            next_stop_id = *stop_id;
        }

        let (trip, stop_id) = backtracking[i as usize].get(&next_stop_id).unwrap();
        let stop_1 = data_storage.stops().find(*stop_id);
        let stop_2 = data_storage.stops().find(next_stop_id);
        next_stop_id = *stop_id;
        let trip = data_storage.trips().find(*trip);
        let tra = trip
            .route()
            .iter()
            .find(|x| x.stop_id() == stop_1.id())
            .unwrap();
        let tra_1 = tra.arrival_time();
        let tra_2 = tra.departure_time();
        let trb = trip
            .route()
            .iter()
            .find(|x| x.stop_id() == stop_2.id())
            .unwrap();
        let trb_1 = trb.arrival_time();
        let trb_2 = trb.departure_time();
        println!(
            "{} : {:?} {:?} => {} : {:?} {:?}",
            stop_1.name(),
            tra_1,
            tra_2,
            stop_2.name(),
            trb_1,
            trb_2,
        );

        i -= 1;
    }

    None
}

fn get_routes(data_storage: &DataStorage) -> Vec<RrRoute> {
    let mut routes = FxHashMap::default();

    for trip in data_storage.trips().entries() {
        let route = routes.entry(trip.hash_route()).or_insert(RrRoute::new());
        route.add_trip(trip.id());

        for route_entry in trip.route() {
            if let Some(departure_time) = route_entry.departure_time() {
                route
                    .trip_departure_times_mut()
                    .insert((trip.id(), route_entry.stop_id()), *departure_time);
            }

            if let Some(arrival_time) = route_entry.arrival_time() {
                route
                    .trip_arrival_times_mut()
                    .insert((trip.id(), route_entry.stop_id()), *arrival_time);
            }
        }
    }

    for (_, route) in &mut routes {
        let first_trip = data_storage.trips().find(*route.trips().first().unwrap());

        for route_entry in first_trip.route() {
            route.add_stop(route_entry.stop_id());
        }

        route.update_id(data_storage);

        route.trips_mut().sort_by(|trip_a_id, trip_b_id| {
            let trip_a = data_storage.trips().find(*trip_a_id);
            let trip_b = data_storage.trips().find(*trip_b_id);

            trip_a
                .route()
                .first()
                .unwrap()
                .departure_time()
                .unwrap()
                .cmp(&trip_b.route().first().unwrap().departure_time().unwrap())
        });
    }

    routes.into_values().collect()
}

fn create_routes_by_stop_id<'a>(
    data_storage: &'a DataStorage,
    routes: &'a Vec<RrRoute>,
) -> FxHashMap<i32, Vec<&'a RrRoute>> {
    routes.iter().fold(FxHashMap::default(), |mut acc, route| {
        let trip = data_storage.trips().find(*route.trips().first().unwrap());

        trip.route().iter().for_each(|route_entry| {
            let stop_routes = acc.entry(route_entry.stop_id()).or_insert(Vec::new());
            stop_routes.push(route);
        });

        acc
    })
}
