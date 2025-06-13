mod models;
mod print;
mod storage;

use std::cmp::min;

use crate::routing::models::{AlgorithmState, RrRoute, StopIndex};
use chrono::Duration;
use rustc_hash::{FxHashMap, FxHashSet};

pub use models::AlgorithmArgs;
pub use storage::RoutingData;

/// Finds the fastest route from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(args: AlgorithmArgs) {
    let mut state = AlgorithmState::new(&args);

    loop {
        state.labels_mut().push(FxHashMap::default());
        state.predecessors_mut().push(FxHashMap::default());

        let routes = get_routes(&args, &state);

        if args.verbose() {
            println!("{} : {}", state.current_round(), routes.len());
        }

        state.marked_stops_mut().clear();

        scan_routes(&args, &mut state, routes);
        scan_transfers(&args, &mut state);

        if state.marked_stops().is_empty() {
            break;
        }

        state.next_round();
    }

    if args.verbose() {
        println!("\nDeparture at: {}\n", args.departure_at());

        let mut least_trips_label_index = 0;

        for k in 0..state.labels().len() {
            println!("{:?}", state.labels()[k].get(&args.arrival_stop_index()));

            if state.labels()[k].get(&args.arrival_stop_index()).is_some()
                && least_trips_label_index == 0
            {
                least_trips_label_index = k;
            }
        }
    }
}

fn get_routes<'a>(
    args: &'a AlgorithmArgs,
    state: &AlgorithmState,
) -> FxHashMap<usize, (&'a RrRoute, usize)> {
    let mut routes = FxHashMap::default();

    state.marked_stops().iter().for_each(|&stop_index| {
        let stop = &args.routing_data().stops()[stop_index];

        for &route_index in stop.routes() {
            let route = &args.routing_data().routes()[route_index];
            let local_stop_index = route.local_stop_index_by_stop_index()[&stop_index];

            routes
                .entry(route_index)
                .and_modify(|entry: &mut (&RrRoute, usize)| {
                    if local_stop_index < entry.1 {
                        *entry = (route, local_stop_index);
                    }
                })
                .or_insert((route, local_stop_index));
        }
    });

    routes
}

fn scan_routes(
    args: &AlgorithmArgs,
    state: &mut AlgorithmState,
    routes: FxHashMap<usize, (&RrRoute, StopIndex)>,
) {
    for (_, (route, stop_local_index)) in routes {
        let mut current_trip_index = None;
        let mut current_trip_boarded_at_stop_index = None;

        for stop_i_local_index in stop_local_index..route.stops().len() {
            // Index in stops Vec.
            let stop_i_index = route.stops()[stop_i_local_index];

            if let Some(trip_index) = current_trip_index {
                let arrival_time_at_stop_i =
                    route.arrival_time(trip_index, stop_i_local_index).unwrap();

                // Case: Stop A (23:54), Stop B (00:04), ...
                // TODO:
                if arrival_time_at_stop_i < args.departure_at().time() {
                    break;
                }

                let earliest_arrival_time_stop_i =
                    state.earliest_arrival_times().get(&stop_i_index);
                let earliest_arrival_time_arrival_stop = state
                    .earliest_arrival_times()
                    .get(&args.arrival_stop_index());

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
                    state.set_label(stop_i_index, arrival_time_at_stop_i);
                    state
                        .earliest_arrival_times_mut()
                        .insert(stop_i_index, arrival_time_at_stop_i);
                    state.mark_stop(stop_i_index);

                    state.set_predecessor(
                        stop_i_index,
                        trip_index,
                        current_trip_boarded_at_stop_index.unwrap(),
                    );
                }
            }

            if stop_i_local_index == route.stops().len() - 1 {
                // It is not possible to board another trip, as the current stop is the terminus.
                continue;
            }

            let previous_arrival_time_at_stop_i =
                state.labels()[state.current_round() - 1].get(&stop_i_index);

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

                if departure_time.unwrap() < args.departure_at().time() {
                    // Case: Stop A (23:54), Stop B (00:04), ...
                    continue;
                }

                if departure_time.unwrap() >= *previous_arrival_time_at_stop_i.unwrap() {
                    current_trip_index = Some(i);
                    current_trip_boarded_at_stop_index = Some(stop_i_index);
                } else {
                    break;
                }
            }
        }
    }
}

fn scan_transfers(algorithm_args: &AlgorithmArgs, algorithm_state: &mut AlgorithmState) {
    let mut additional_marked_stops = FxHashSet::default();
    let marked_stops: Vec<_> = algorithm_state.marked_stops().iter().cloned().collect();

    for stop_index in marked_stops {
        let stop = &algorithm_args.routing_data().stops()[stop_index];

        for transfer in stop.transfers() {
            let arrival_time_1 = algorithm_state.label(transfer.other_stop_index());
            let arrival_time_2 = algorithm_state.label(stop_index).unwrap() + transfer.duration();

            let arrival_time = arrival_time_1.map_or(arrival_time_2, |arrival_time_1| {
                min(arrival_time_1, arrival_time_2)
            });

            // TODO:
            if arrival_time < algorithm_args.departure_at().time() {
                continue;
            }

            algorithm_state.set_label(transfer.other_stop_index(), arrival_time);
            additional_marked_stops.insert(transfer.other_stop_index());
        }
    }

    algorithm_state
        .marked_stops_mut()
        .extend(additional_marked_stops);
}
