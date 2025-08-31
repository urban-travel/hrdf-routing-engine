use chrono::NaiveDateTime;
use hrdf_parser::DataStorage;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::utils::add_minutes_to_date_time;

use super::{
    connections::get_connections,
    models::{Route, RouteSection},
    utils::{clone_update_route, get_stop_connections, sort_routes, sorted_insert},
};

pub fn explore_routes<F>(
    data_storage: &DataStorage,
    mut routes: Vec<Route>,
    journeys_to_ignore: &mut FxHashSet<i32>,
    earliest_arrival_by_stop_id: &mut FxHashMap<i32, NaiveDateTime>,
    mut can_continue_exploration: F,
) -> Vec<Route>
where
    F: FnMut(&Route) -> bool,
{
    let mut new_routes = Vec::new();

    let mut old_routes = routes.clone();
    while !routes.is_empty() {
        let route = routes.remove(0);

        if !can_continue_exploration(&route) {
            continue;
        }

        if route.last_section().departure_stop_id() == route.last_section().arrival_stop_id() {
            // Some journeys start and end at the same stop, so it's not possible to know whether the journey has reached its last stop.
            // The above condition, however, lets us know that the journey is about to loop.
            continue;
        }

        explore_last_route_section_more_if_possible(data_storage, &route, &mut routes);

        if !can_explore_connections(data_storage, &route, earliest_arrival_by_stop_id) {
            // In some cases there are stops appearing multiple times in a Journey
            // for example see: *Z 011709 000801   in FPLAHN
            // This can lead to an infinite loop. We will therefore check if the same route is explored
            // a second time
            if routes.len() == 1 && routes == old_routes {
                log::debug!("Routes stayed the same: {:?}", routes);
                let _ = routes.remove(0);
            } else if routes.len() == 1 {
                old_routes = routes.clone();
            }
            continue;
        }

        // log::debug!("EXPLORE NEARBY STOPS");
        explore_nearby_stops(data_storage, &route, &mut routes);
        // log::debug!("EXPLRED NEARBY STOPS: {:?}", routes);
        explore_connections(data_storage, &route, journeys_to_ignore, &mut new_routes);
        // log::debug!("EXPLRED CONNECTIONS: {:?}", new_routes);

        // log::debug!("==============================================================");
    }

    // All new journeys are recorded as not available for the next connection level.
    new_routes.iter().for_each(|route| {
        if let Some(journey_id) = route.last_section().journey_id() {
            journeys_to_ignore.insert(journey_id);
        }
    });

    sort_routes(&mut new_routes);
    new_routes
}

fn explore_last_route_section_more_if_possible(
    data_storage: &DataStorage,
    route: &Route,
    routes: &mut Vec<Route>,
) {
    let Some(journey_id) = route.last_section().journey_id() else {
        return;
    };

    // The next section (tronçon dans ce cas) is visited if possible.
    let new_route = route.extend(data_storage, journey_id, route.arrival_at().date(), false);

    if let Some(rou) = new_route {
        sorted_insert(routes, rou);
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
        log::warn!("Stop: {} not found.", stop_id);
        return false;
    };

    if !stop.can_be_used_as_exchange_point() {
        // The arrival stop of the last RouteSection of a journey is not necessarily usable for exchange, hence the check.
        return false;
    }

    let arrival_at = route.arrival_at();

    if let Some(&earliest_arrival) = earliest_arrival_by_stop_id.get(&stop_id) {
        if arrival_at < earliest_arrival {
            // The route arrived even earlier than the last route recorded for the stop.
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

fn explore_connections(
    data_storage: &DataStorage,
    route: &Route,
    journeys_to_ignore: &FxHashSet<i32>,
    new_routes: &mut Vec<Route>,
) {
    new_routes.extend(get_connections(data_storage, route, journeys_to_ignore));
}

fn explore_nearby_stops(data_storage: &DataStorage, route: &Route, routes: &mut Vec<Route>) {
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
    .for_each(|new_route| sorted_insert(routes, new_route));
}
