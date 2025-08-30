use hrdf_parser::{DataStorage, StopConnection};
use rustc_hash::FxHashSet;

use super::models::{Route, RouteSection};

pub fn clone_update_route<F>(route: &Route, f: F) -> Route
where
    F: FnOnce(&mut Vec<RouteSection>, &mut FxHashSet<i32>),
{
    let mut cloned_sections = route.sections().clone();
    let mut cloned_visited_stops = route.visited_stops().clone();

    f(&mut cloned_sections, &mut cloned_visited_stops);

    Route::new(cloned_sections, cloned_visited_stops)
}

pub fn get_stop_connections(
    data_storage: &DataStorage,
    stop_id: i32,
) -> Option<Vec<&StopConnection>> {
    data_storage
        .stop_connections_by_stop_id()
        .get(&stop_id)
        // .map(|ids| data_storage.stop_connections().resolve_ids(ids))?
        .map(|ids| {
            data_storage
                .stop_connections()
                .resolve_ids(ids)
                .unwrap_or_else(|| panic!("Ids {:?} not found.", ids))
        })
}

pub fn get_routes_to_ignore(data_storage: &DataStorage, route: &Route) -> FxHashSet<u64> {
    route
        .sections()
        .iter()
        .filter_map(|section| {
            section
                .journey(data_storage)
                .and_then(|journey| journey.hash_route(route.arrival_stop_id()))
        })
        .collect()
}

pub fn sort_routes(routes: &mut [Route]) {
    routes.sort_by_key(|route| route.arrival_at());
}

pub fn sorted_insert(routes: &mut Vec<Route>, route_to_insert: Route) {
    let index = routes
        .iter_mut()
        .position(|route| route_to_insert.arrival_at() < route.arrival_at())
        .unwrap_or(routes.len());
    routes.insert(index, route_to_insert);
}
