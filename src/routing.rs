mod models;
mod print;
mod storage;

use crate::routing::models::{AlgorithmState, Journey, Leg, RrRoute, StopIndex};
use chrono::{Local, NaiveDateTime};
use hrdf_parser::Model;
use rustc_hash::{FxHashMap, FxHashSet};

pub use models::AlgorithmArgs;
pub use storage::RoutingData;

/// Finds the fastest route from the departure stop to the arrival stop.
/// The departure date and time must be within the timetable period.
pub fn plan_journey(args: AlgorithmArgs) -> Journey {
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
        println!();
    }

    for k in 1..state.labels().len() {
        if args.verbose() {
            println!(
                "{k} : {:?}",
                state.labels()[k].get(&args.arrival_stop_index())
            );
        }
    }

    let earliest_arrival_time_round = (1..state.labels().len())
        .filter(|&k| state.labels()[k].contains_key(&args.arrival_stop_index()))
        .min_by_key(|&k| state.labels()[k][&args.arrival_stop_index()]);

    if args.verbose() {
        println!("\n{:?}", earliest_arrival_time_round);

        if earliest_arrival_time_round.is_none() {
            println!("No solution found");
        }
    }

    let mut legs = Vec::new();
    let mut destination_stop_index = args.arrival_stop_index();
    let mut i = earliest_arrival_time_round.unwrap() - 1;

    loop {
        let (trip_id, origin_stop_index) = state.predecessors()[i][&destination_stop_index];

        let stop_origin = args
            .routing_data()
            .data_storage()
            .stops()
            .find(args.routing_data().stops()[origin_stop_index].id());
        let stop_destination = args
            .routing_data()
            .data_storage()
            .stops()
            .find(args.routing_data().stops()[destination_stop_index].id());

        if trip_id == 0 {
            legs.push(Leg::new(
                None,
                stop_origin.id(),
                None,
                stop_destination.id(),
                None,
                Some(0),
            ));
        } else {
            let trip = args.routing_data().data_storage().trips().find(trip_id);

            let departure_at = trip
                .route()
                .iter()
                .find(|x| x.stop_id() == stop_origin.id())
                .and_then(|stop| *stop.departure_time())
                .map(|time| NaiveDateTime::new(args.departure_at().date(), time));

            let arrival_at = trip
                .route()
                .iter()
                .find(|x| x.stop_id() == stop_destination.id())
                .and_then(|stop| *stop.arrival_time())
                .map(|time| NaiveDateTime::new(args.departure_at().date(), time));

            legs.push(Leg::new(
                Some(trip.id()),
                stop_origin.id(),
                departure_at,
                stop_destination.id(),
                arrival_at,
                Some(0),
            ));
        }

        destination_stop_index = origin_stop_index;

        if trip_id != 0 {
            if i == 0 {
                break;
            }

            i -= 1;
        }
    }

    Journey::new(
        Local::now().naive_local(),
        Local::now().naive_local(),
        legs.into_iter().rev().collect(),
    )
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
                let break_loop = evaluate_stop(
                    args,
                    state,
                    route,
                    trip_index,
                    current_trip_boarded_at_stop_index.unwrap(),
                    stop_i_index,
                    stop_i_local_index,
                );

                if break_loop {
                    break;
                }
            }

            if stop_i_local_index == route.stops().len() - 1 {
                // It is not possible to board another trip, as the current stop is the terminus.
                continue;
            }

            (current_trip_index, current_trip_boarded_at_stop_index) = try_catch_earlier_trip(
                args,
                state,
                route,
                current_trip_index,
                current_trip_boarded_at_stop_index,
                stop_i_index,
                stop_i_local_index,
            );
        }
    }
}

fn evaluate_stop(
    args: &AlgorithmArgs,
    state: &mut AlgorithmState,
    route: &RrRoute,
    trip_index: usize,
    trip_boarded_at_stop_index: StopIndex,
    stop_index: usize,
    stop_local_index: usize,
) -> bool {
    let arrival_time = route.arrival_time(trip_index, stop_local_index).unwrap();

    // Case: Stop A (23:54), Stop B (00:04), ...
    // TODO:
    if arrival_time < args.departure_at().time() {
        return true;
    }

    let can_label_be_improved = match (
        state.earliest_arrival_time(stop_index),
        state.earliest_arrival_time(args.arrival_stop_index()),
    ) {
        (None, None) => true,
        (Some(arrival_time_1), None) => arrival_time < arrival_time_1,
        (None, Some(arrival_time_2)) => arrival_time < arrival_time_2,
        (Some(arrival_time_1), Some(arrival_time_2)) => {
            arrival_time < arrival_time_1.min(arrival_time_2)
        }
    };

    if can_label_be_improved {
        state.set_label(stop_index, arrival_time);
        state.set_earliest_arrival_time(stop_index, arrival_time);
        state.mark_stop(stop_index);

        state.set_predecessor(
            stop_index,
            route.trips()[trip_index].id(),
            trip_boarded_at_stop_index,
        );
    }

    false
}

fn try_catch_earlier_trip(
    args: &AlgorithmArgs,
    state: &mut AlgorithmState,
    route: &RrRoute,
    mut trip_index: Option<usize>,
    mut trip_boarded_at_stop_index: Option<StopIndex>,
    stop_index: usize,
    stop_local_index: usize,
) -> (Option<usize>, Option<StopIndex>) {
    let previous_arrival = state.previous_label(stop_index);

    let can_catch = match (previous_arrival, trip_index) {
        (Some(prev_arr), Some(trip_index)) => route
            .departure_time(trip_index, stop_local_index)
            .map_or(false, |dep| prev_arr <= dep),
        (Some(_), None) => true,
        _ => false,
    };

    if !can_catch {
        return (trip_index, trip_boarded_at_stop_index);
    }

    let start = trip_index.unwrap_or_else(|| route.trips().len());

    for i in (0..start).rev() {
        let Some(departure_time) = route.departure_time(i, stop_local_index) else {
            continue;
        };

        // TODO:
        if departure_time < args.departure_at().time() {
            // Case: Stop A (23:54), Stop B (00:04), ...
            continue;
        }

        if departure_time >= previous_arrival.unwrap() {
            trip_index = Some(i);
            trip_boarded_at_stop_index = Some(stop_index);
        } else {
            break;
        }
    }

    (trip_index, trip_boarded_at_stop_index)
}

fn scan_transfers(args: &AlgorithmArgs, state: &mut AlgorithmState) {
    let mut additional_marked_stops = FxHashSet::default();
    let marked_stops: Vec<_> = state.marked_stops().iter().cloned().collect();

    for stop_index in marked_stops {
        let stop = &args.routing_data().stops()[stop_index];

        for transfer in stop.transfers() {
            let arrival_time_candidate = state.label(stop_index).unwrap() + transfer.duration();

            // TODO:
            if arrival_time_candidate < args.departure_at().time() {
                continue;
            }

            if let Some(current_best_arrival_time) = state.label(transfer.other_stop_index()) {
                if arrival_time_candidate < current_best_arrival_time {
                    state.set_label(transfer.other_stop_index(), arrival_time_candidate);
                    state.set_predecessor(transfer.other_stop_index(), 0, stop_index);
                }
            } else {
                state.set_label(transfer.other_stop_index(), arrival_time_candidate);
                state.set_predecessor(transfer.other_stop_index(), 0, stop_index);
            }

            additional_marked_stops.insert(transfer.other_stop_index());
        }
    }

    state.marked_stops_mut().extend(additional_marked_stops);
}
