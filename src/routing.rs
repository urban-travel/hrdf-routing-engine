mod models;
mod print;

use std::time::Instant;

use hrdf_parser::{DataStorage, Hrdf, Model};

use chrono::NaiveDateTime;
use models::Journey;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::routing::models::RrRoute;

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

    let mut k = 1;
    loop {
        println!("{}", k);

        labels.push(FxHashMap::default());
        let mut routes_k = FxHashMap::default();

        for stop_id in &marked_stops {
            for route in routes_by_stop_id.get(&stop_id).unwrap() {
                if let Some(value) = routes_k.get_mut(route) {
                    let trip = data_storage.trips().find(*route.trips().first().unwrap());

                    let i = trip.route().iter().position(|x| x.stop_id() == *stop_id);
                    let j = trip.route().iter().position(|x| x.stop_id() == *value);

                    if i < j {
                        *value = *stop_id;
                    }
                } else {
                    routes_k.insert(route, *stop_id);
                }
            }
        }

        marked_stops.clear();
        println!("{}", routes_k.len());

        for (route, stop_id) in routes_k {
            let mut current_trip_id = None;

            let stops = data_storage
                .trips()
                .find(*route.trips().first().unwrap())
                .route();
            let stops = stops
                .iter()
                .skip(
                    stops
                        .iter()
                        .position(|route_entry| route_entry.stop_id() == stop_id)?,
                )
                .map(|x| x.stop_id());

            for stop_id_i in stops {
                if let Some(trip_id) = current_trip_id {
                    let trip = data_storage.trips().find(trip_id);
                    let arrival_time = trip
                        .route()
                        .iter()
                        .skip(1)
                        .find(|x| x.stop_id() == stop_id_i)
                        .unwrap()
                        .arrival_time()
                        .unwrap();

                    // Case: Stop A (23:54), Stop B (00:04), ...
                    if arrival_time > trip.route().first().unwrap().departure_time().unwrap() {
                        let a = earliest_known_arrival_time_by_stop_id.get(&stop_id_i);
                        let b = earliest_known_arrival_time_by_stop_id.get(&arrival_stop_id);
                        let c = a.into_iter().chain(b).min();

                        if c.map_or(true, |t| arrival_time < *t) {
                            labels.last_mut().unwrap().insert(stop_id_i, arrival_time);
                            earliest_known_arrival_time_by_stop_id.insert(stop_id_i, arrival_time);
                            marked_stops.insert(stop_id_i);
                        }
                    }
                }

                let previous_best_arrival_time = labels[k - 1].get(&stop_id_i);

                let can_catch_earlier_trip =
                    previous_best_arrival_time.map_or(false, |arrival_time| {
                        current_trip_id.map_or(true, |trip_id| {
                            let trip_departure_time = data_storage
                                .trips()
                                .find(trip_id)
                                .route()
                                .iter()
                                .find(|r| r.stop_id() == stop_id_i)
                                .unwrap()
                                .departure_time();
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
                            let departure_time = data_storage
                                .trips()
                                .find(trip_id)
                                .route()
                                .iter()
                                .find(|route_entry| route_entry.stop_id() == stop_id_i)
                                .unwrap()
                                .departure_time();
                            if let Some(departure_time) = departure_time {
                                *departure_time >= *previous_best_arrival_time
                            } else {
                                false
                            }
                        })
                        .cloned();
                }
            }
        }

        if marked_stops.is_empty() {
            break;
        }

        k += 1;
    }

    println!("\n{:.2?}", start_time.elapsed());
    println!("{}", departure_at);
    println!(
        "{}",
        earliest_known_arrival_time_by_stop_id
            .get(&arrival_stop_id)
            .unwrap()
    );
    println!("---");
    for i in 0..labels.len() {
        println!("{:?}", labels[i].get(&arrival_stop_id));
    }

    None
}

fn get_routes(data_storage: &DataStorage) -> Vec<RrRoute> {
    let mut routes = FxHashMap::default();

    for trip in data_storage.trips().entries() {
        let route = routes.entry(trip.hash_route()).or_insert(RrRoute::new());
        route.add_trip(trip.id());
    }

    for (_, route) in &mut routes {
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
