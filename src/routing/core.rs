use chrono::NaiveDateTime;
use hrdf_parser::DataStorage;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::utils::add_minutes_to_date_time;

use super::{
    connections::next_departures,
    exploration::explore_routes,
    models::{Route, RouteResult, RouteSection, RoutingAlgorithmArgs, RoutingAlgorithmMode},
    utils::{RouteQueue, get_stop_connections},
};

pub fn compute_routing(
    data_storage: &DataStorage,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
    max_num_explorable_connections: i32,
    verbose: bool,
    args: RoutingAlgorithmArgs,
) -> FxHashMap<i32, RouteResult> {
    let mut routes = create_initial_routes(data_storage, departure_stop_id, departure_at);
    let mut earliest_arrival_by_stop_id = FxHashMap::default();
    let mut solutions = FxHashMap::default();

    let mut journeys_to_ignore = routes
        .iter_routes()
        .filter_map(|route| route.last_section().journey_id())
        .collect::<FxHashSet<_>>();

    for i in 0..max_num_explorable_connections {
        if verbose {
            log::info!("For connection {i}, routes length: {}", routes.len());
        }

        let can_continue_exploration: Box<dyn FnMut(&Route) -> bool> = match args.mode() {
            RoutingAlgorithmMode::SolveFromDepartureStopToArrivalStop => Box::new(|route| {
                can_continue_exploration_one_to_one(
                    data_storage,
                    route,
                    &mut solutions,
                    args.arrival_stop_id(),
                )
            }),
            RoutingAlgorithmMode::SolveFromDepartureStopToReachableArrivalStops => {
                Box::new(|route| {
                    can_continue_exploration_one_to_many(
                        data_storage,
                        route,
                        &mut solutions,
                        args.time_limit(),
                    )
                })
            }
        };

        let new_routes = explore_routes(
            data_storage,
            routes,
            &mut journeys_to_ignore,
            &mut earliest_arrival_by_stop_id,
            can_continue_exploration,
        );

        if new_routes.is_empty() {
            break;
        }

        routes = new_routes;
    }

    solutions
        .into_iter()
        .map(|(k, v)| (k, v.to_route_result(data_storage)))
        .collect()
}

pub fn create_initial_routes(
    data_storage: &DataStorage,
    departure_stop_id: i32,
    departure_at: NaiveDateTime,
) -> RouteQueue {
    let mut routes = RouteQueue::new();

    for (journey, journey_departure_at) in
        next_departures(data_storage, departure_stop_id, departure_at, None, None)
    {
        if let Some((section, mut visited_stops)) = RouteSection::find_next(
            data_storage,
            journey,
            departure_stop_id,
            journey_departure_at.date(),
            true,
        ) {
            visited_stops.insert(departure_stop_id);
            routes.push(Route::new(vec![section], visited_stops));
        }
    }

    if let Some(stop_connections) = get_stop_connections(data_storage, departure_stop_id) {
        stop_connections.iter().for_each(|stop_connection| {
            let mut visited_stops = FxHashSet::default();
            visited_stops.insert(stop_connection.stop_id_1());
            visited_stops.insert(stop_connection.stop_id_2());

            let section = RouteSection::new(
                None,
                stop_connection.stop_id_1(),
                stop_connection.stop_id_2(),
                add_minutes_to_date_time(departure_at, stop_connection.duration().into()),
                Some(stop_connection.duration()),
            );
            routes.push(Route::new(vec![section], visited_stops));
        });
    }

    routes
}

fn can_continue_exploration_one_to_one(
    data_storage: &DataStorage,
    route: &Route,
    solutions: &mut FxHashMap<i32, Route>,
    arrival_stop_id: i32,
) -> bool {
    if !route.visited_stops().contains(&arrival_stop_id) {
        let solution = solutions.get(&arrival_stop_id);
        return can_improve_solution(route, &solution);
    }

    let solution = solutions.get(&arrival_stop_id);
    let candidate = if route.last_section().journey_id().is_none() {
        route.clone()
    } else {
        update_arrival_stop(data_storage, route.clone(), arrival_stop_id)
    };

    if is_improving_solution(data_storage, &candidate, &solution) {
        solutions.insert(arrival_stop_id, candidate);
    }

    false
}

fn can_continue_exploration_one_to_many(
    data_storage: &DataStorage,
    route: &Route,
    solutions: &mut FxHashMap<i32, Route>,
    time_limit: NaiveDateTime,
) -> bool {
    fn evaluate_candidate(
        data_storage: &DataStorage,
        candidate: Route,
        solutions: &mut FxHashMap<i32, Route>,
        time_limit: NaiveDateTime,
    ) {
        if candidate.arrival_at() > time_limit {
            return;
        }

        let arrival_stop_id = candidate.arrival_stop_id();
        let solution = solutions.get(&arrival_stop_id);

        if is_improving_solution(data_storage, &candidate, &solution) {
            solutions.insert(arrival_stop_id, candidate);
        }
    }

    if route.last_section().journey_id().is_none() {
        evaluate_candidate(data_storage, route.clone(), solutions, time_limit);
    } else {
        let last_section = route.last_section();
        let journey = last_section.journey(data_storage).unwrap();

        for route_entry in journey.route_section(
            last_section.departure_stop_id(),
            last_section.arrival_stop_id(),
        ) {
            let candidate = update_arrival_stop(data_storage, route.clone(), route_entry.stop_id());
            evaluate_candidate(data_storage, candidate, solutions, time_limit);
        }
    }

    route.arrival_at() < time_limit
}

/// Do not call this function if route.last_section().journey_id() is None.
fn update_arrival_stop(
    data_storage: &DataStorage,
    mut route: Route,
    arrival_stop_id: i32,
) -> Route {
    let last_section = route.last_section();

    let journey = last_section.journey(data_storage).unwrap();
    let arrival_at = journey
        .arrival_at_of_with_origin(
            arrival_stop_id,
            last_section.arrival_at().date(),
            false,
            last_section.arrival_stop_id(),
        )
        .unwrap_or_else(|_| {
            panic!(
                "Arrival at not found for {arrival_stop_id}, {}, and {}",
                last_section.arrival_at().date(),
                last_section.arrival_stop_id()
            )
        });

    let last_section = route.last_section_mut();
    last_section.set_arrival_stop_id(arrival_stop_id);
    last_section.set_arrival_at(arrival_at);

    route
}

fn can_improve_solution(route: &Route, solution: &Option<&Route>) -> bool {
    solution
        .as_ref()
        .is_none_or(|sol| route.arrival_at() <= sol.arrival_at())
}

fn is_improving_solution(
    data_storage: &DataStorage,
    candidate: &Route,
    solution: &Option<&Route>,
) -> bool {
    fn count_stops(data_storage: &DataStorage, section: &RouteSection) -> usize {
        section
            .journey(data_storage)
            .unwrap()
            .count_stops(section.departure_stop_id(), section.arrival_stop_id())
    }

    if candidate.sections().len() == 1 && candidate.last_section().journey_id().is_none() {
        // If the candidate contains only a walking trip, it is not a valid solution.
        return false;
    }

    if solution.is_none() {
        // If this is the first solution found, then we keep the candidate as the solution.
        return true;
    }

    let solution = solution.unwrap();

    // A variable suffixed with 1 will always correspond to the candiate, suffixed with 2 will correspond to the solution.
    let t1 = candidate.arrival_at();
    let t2 = solution.arrival_at();

    if t1 != t2 {
        // If the candidate arrives earlier than the solution, then it is a better solution.
        return t1 < t2;
    }

    let connection_count_1 = candidate.count_connections();
    let connection_count_2 = solution.count_connections();

    if connection_count_1 != connection_count_2 {
        // If the candidate requires fewer connections, then it is a better solution.
        return connection_count_1 < connection_count_2;
    }

    let sections_1 = candidate.sections_having_journey();
    let sections_2 = solution.sections_having_journey();

    // Compare each connection.
    for i in 0..connection_count_1 {
        let stop_count_1 = count_stops(data_storage, sections_1[i]);
        let stop_count_2 = count_stops(data_storage, sections_2[i]);

        if stop_count_1 != stop_count_2 {
            // If the candidate crosses more stops than the solution, then it is a better solution.
            return stop_count_1 > stop_count_2;
        }
    }

    // The current solution is better than the candidate.
    false
}
