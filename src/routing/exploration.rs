use chrono::NaiveDateTime;
use hrdf_parser::DataStorage;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::utils::add_minutes_to_date_time;

use super::{
    connections::{
        ArrivalCache, DepartureCache, ReverseConnectionTimes, get_connections,
        get_connections_reverse,
    },
    core::is_improving_solution_reverse,
    models::{Route, RouteSection},
    utils::{RouteQueue, RouteQueueReverse, clone_update_route, get_stop_connections},
};

pub fn explore_routes<'a, F>(
    data_storage: &'a DataStorage,
    mut routes: RouteQueue,
    journeys_to_ignore: &mut FxHashSet<i32>,
    earliest_arrival_by_stop_id: &mut FxHashMap<i32, NaiveDateTime>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    departure_cache: &DepartureCache<'a>,
    mut can_continue_exploration: F,
) -> RouteQueue
where
    F: FnMut(&Route) -> bool,
{
    let mut new_routes = RouteQueue::new();
    let mut visited_routes = FxHashSet::default();

    while let Some(route) = routes.pop() {
        if !can_continue_exploration(&route) {
            continue;
        }

        if route.last_section().departure_stop_id() == route.last_section().arrival_stop_id() {
            // Some journeys start and end at the same stop, so it's not possible to know whether the journey has reached its last stop.
            // The above condition, however, lets us know that the journey is about to loop.
            continue;
        }

        let can_continue =
            can_explore_connections(data_storage, &route, earliest_arrival_by_stop_id);

        if !can_continue {
            // In some cases there are stops appearing multiple times in a Journey
            // for example see: *Z 011709 000801   in FPLAHN
            // Extending such a route can reproduce an identical Route forever. Detect the
            // repeat and stop extending *this* route, instead of discarding an unrelated
            // one from the queue.
            if visited_routes.contains(&route) {
                log::info!("Routes stayed the same: {}", routes.len());
                visited_routes.remove(&route);
                continue;
            }
            visited_routes.insert(route.clone());
        }

        explore_last_route_section_more_if_possible(data_storage, &route, &mut routes);

        if !can_continue {
            continue;
        }

        explore_nearby_stops(data_storage, &route, &mut routes);
        explore_connections(
            data_storage,
            &route,
            journeys_to_ignore,
            hash_route_cache,
            departure_cache,
            &mut new_routes,
        );
    }

    // All new journeys are recorded as not available for the next connection level.
    new_routes.iter_routes().for_each(|route| {
        if let Some(journey_id) = route.last_section().journey_id() {
            journeys_to_ignore.insert(journey_id);
        }
    });

    new_routes
}

fn explore_last_route_section_more_if_possible(
    data_storage: &DataStorage,
    route: &Route,
    routes: &mut RouteQueue,
) {
    let Some(journey_id) = route.last_section().journey_id() else {
        return;
    };

    // The next section (tronçon dans ce cas) is visited if possible.
    let new_route = route.extend(data_storage, journey_id, route.arrival_at().date(), false);

    if let Some(rou) = new_route {
        // A journey can visit the same stop several times (for example see: *Z 011709 000801
        // in FPLAHN), in which case extending the route can give back the very same route.
        // Pushing it would make it be popped, extended and pushed again forever.
        if rou != *route {
            routes.push(rou);
        }
    }
}

fn can_explore_connections(
    data_storage: &DataStorage,
    route: &Route,
    earliest_arrival_by_stop_id: &mut FxHashMap<i32, NaiveDateTime>,
) -> bool {
    let stop_id = route.arrival_stop_id();
    let stop = data_storage.stops().find(stop_id);
    let stop = if let Some(stop) = stop {
        stop
    } else {
        log::debug!("Stop: {} not found.", stop_id);
        return false;
    };

    if !stop.can_be_used_as_exchange_point() {
        // The arrival stop of the last RouteSection of a journey is not necessarily usable for exchange, hence the check.
        return false;
    }

    let arrival_at = route.arrival_at();

    if let Some(&earliest_arrival) = earliest_arrival_by_stop_id.get(&stop_id) {
        if arrival_at <= earliest_arrival {
            // The route arrived at least as early as the best route recorded for the stop.
            // Using <= (not <) matters: two different paths can legitimately arrive at the
            // exact same stop at the exact same time, and discarding one of them purely
            // because it was processed second would silently drop a potentially better
            // continuation.
            earliest_arrival_by_stop_id.insert(stop_id, arrival_at);
            true
        } else {
            // Another route reached the stop faster.
            false
        }
    } else {
        // This is the first time the stop has been found.
        earliest_arrival_by_stop_id.insert(stop_id, arrival_at);
        true
    }
}

fn explore_connections<'a>(
    data_storage: &'a DataStorage,
    route: &Route,
    journeys_to_ignore: &FxHashSet<i32>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    departure_cache: &DepartureCache<'a>,
    new_routes: &mut RouteQueue,
) {
    for route in get_connections(
        data_storage,
        route,
        journeys_to_ignore,
        hash_route_cache,
        departure_cache,
    ) {
        new_routes.push(route);
    }
}

fn explore_nearby_stops(data_storage: &DataStorage, route: &Route, routes: &mut RouteQueue) {
    if route.last_section().journey_id().is_none() {
        // No walking between 2 stops, after walking between 2 stops just before.
        return;
    }
    match get_stop_connections(data_storage, route.arrival_stop_id()) {
        Some(stop_connections) => stop_connections,
        None => return,
    }
    .into_iter()
    // Sometimes certain stop identifiers don't exist for unknown reasons.
    .filter(|stop_connection| {
        data_storage
            .stops()
            .data()
            .contains_key(&stop_connection.stop_id_2())
    })
    // No return to a previously visited stop.
    .filter(|stop_connection| !route.visited_stops().contains(&stop_connection.stop_id_2()))
    .map(|stop_connection| {
        clone_update_route(route, |cloned_sections, cloned_visited_stops| {
            cloned_sections.push(RouteSection::new(
                None,
                stop_connection.stop_id_1(),
                stop_connection.stop_id_2(),
                add_minutes_to_date_time(route.arrival_at(), stop_connection.duration().into()),
                Some(stop_connection.duration()),
            ));
            cloned_visited_stops.insert(stop_connection.stop_id_2());
        })
    })
    .for_each(|new_route| routes.push(new_route));
}

pub fn explore_routes_reverse<'a, F>(
    data_storage: &'a DataStorage,
    mut routes: RouteQueueReverse,
    journeys_to_ignore: &mut FxHashSet<i32>,
    connection_times: &mut ReverseConnectionTimes,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    arrival_cache: &ArrivalCache<'a>,
    mut can_continue_exploration: F,
) -> RouteQueueReverse
where
    F: FnMut(&Route) -> bool,
{
    let mut new_routes = RouteQueueReverse::new();

    // Several transfers can lead to the same scheduled arrival/departure event.
    // Its continuation has identical transfer times. Revisit it only if the
    // alternative improves the usual transfer-count / stop-count preference.
    let mut explored_events = FxHashMap::default();

    while let Some(route) = routes.pop() {
        if !can_continue_exploration(&route) {
            continue;
        }

        if route.last_section().departure_stop_id() == route.last_section().arrival_stop_id() {
            continue;
        }

        if let Some(journey_id) = route.last_section().journey_id() {
            let event = (journey_id, route.arrival_stop_id(), route.arrival_at());
            if !is_improving_solution_reverse(data_storage, &route, &explored_events.get(&event)) {
                continue;
            }
        }

        explore_last_route_section_more_if_possible_reverse(data_storage, &route, &mut routes);

        if is_exchange_point(data_storage, &route) {
            explore_nearby_stops_reverse(
                data_storage,
                &route,
                &mut routes,
                &mut connection_times.footpaths,
            );
            if connection_times.can_explore(&route) {
                explore_connections_reverse(
                    data_storage,
                    &route,
                    journeys_to_ignore,
                    hash_route_cache,
                    arrival_cache,
                    &mut new_routes,
                );
            }
        }
        if let Some(journey_id) = route.last_section().journey_id() {
            let event = (journey_id, route.arrival_stop_id(), route.arrival_at());
            explored_events.insert(event, route);
        }
    }

    new_routes.iter_routes().for_each(|route| {
        if let Some(journey_id) = route.last_section().journey_id() {
            journeys_to_ignore.insert(journey_id);
        }
    });

    new_routes
}

fn explore_last_route_section_more_if_possible_reverse(
    data_storage: &DataStorage,
    route: &Route,
    routes: &mut RouteQueueReverse,
) {
    let Some(journey_id) = route.last_section().journey_id() else {
        return;
    };

    // The previous section is visited if possible.
    // Note: extend_reverse needs is_arrival_date = true because route.arrival_at() corresponds
    // to the arrival time at the current stop (physically).
    let new_route = route.extend_reverse(data_storage, journey_id, route.arrival_at().date(), true);

    if let Some(rou) = new_route {
        routes.push(rou);
    }
}

fn explore_connections_reverse<'a>(
    data_storage: &'a DataStorage,
    route: &Route,
    journeys_to_ignore: &FxHashSet<i32>,
    hash_route_cache: &mut FxHashMap<(i32, i32), Option<u64>>,
    arrival_cache: &ArrivalCache<'a>,
    new_routes: &mut RouteQueueReverse,
) {
    for route in get_connections_reverse(
        data_storage,
        route,
        journeys_to_ignore,
        hash_route_cache,
        arrival_cache,
    ) {
        new_routes.push(route);
    }
}

fn explore_nearby_stops_reverse(
    data_storage: &DataStorage,
    route: &Route,
    routes: &mut RouteQueueReverse,
    footpath_times: &mut FxHashMap<(i32, i32), NaiveDateTime>,
) {
    if route.last_section().journey_id().is_none() {
        return;
    }
    match get_stop_connections(data_storage, route.arrival_stop_id()) {
        Some(stop_connections) => stop_connections,
        None => return,
    }
    .into_iter()
    .filter(|stop_connection| {
        data_storage
            .stops()
            .data()
            .contains_key(&stop_connection.stop_id_2())
    })
    .filter(|stop_connection| !route.visited_stops().contains(&stop_connection.stop_id_2()))
    .filter(|stop_connection| {
        let key = (route.arrival_stop_id(), stop_connection.stop_id_2());
        let departure_at =
            add_minutes_to_date_time(route.arrival_at(), -(stop_connection.duration() as i64));
        if footpath_times
            .get(&key)
            .is_some_and(|&best| departure_at <= best)
        {
            return false;
        }
        footpath_times.insert(key, departure_at);
        true
    })
    .map(|stop_connection| {
        clone_update_route(route, |cloned_sections, cloned_visited_stops| {
            cloned_sections.push(RouteSection::new(
                None,
                stop_connection.stop_id_1(),
                stop_connection.stop_id_2(),
                // In reverse, we subtract the walking duration to find when we started walking
                add_minutes_to_date_time(route.arrival_at(), -(stop_connection.duration() as i64)),
                Some(stop_connection.duration()),
            ));
            cloned_visited_stops.insert(stop_connection.stop_id_2());
        })
    })
    .for_each(|new_route| routes.push(new_route));
}

fn is_exchange_point(data_storage: &DataStorage, route: &Route) -> bool {
    let stop_id = route.arrival_stop_id();
    let Some(stop) = data_storage.stops().find(stop_id) else {
        log::debug!("Stop: {} not found.", stop_id);
        return false;
    };

    // The frontier of a journey is not necessarily usable for an interchange.
    stop.can_be_used_as_exchange_point()
}
