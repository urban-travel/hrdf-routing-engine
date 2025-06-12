// ------------------------------------------------------------------------------------------------
// --- RrStorage
// ------------------------------------------------------------------------------------------------

use hrdf_parser::DataStorage;
use rustc_hash::FxHashMap;

use crate::routing::models::{RrRoute, RrStop, RrStopTime, RrTransfer};

#[derive(Debug)]
pub struct RrStorage {
    routes: Vec<RrRoute>,
    stop_times: Vec<RrStopTime>,
    route_stops: Vec<usize>,
    stops: Vec<RrStop>,
    stop_routes: Vec<usize>,
    transfers: Vec<RrTransfer>,
}

impl RrStorage {
    pub fn new(data_storage: &DataStorage) -> Self {
        let (routes, stop_times, route_stops) = get_routes(data_storage);
        let (stops, stop_routes, transfers) = get_stops(data_storage, &routes, &route_stops);
        let route_stops = fix_route_stops(route_stops, &stops);

        Self {
            routes,
            stop_times,
            route_stops,
            stops,
            stop_routes,
            transfers,
        }
    }

    // Getters/Setters

    pub fn routes(&self) -> &Vec<RrRoute> {
        &self.routes
    }

    pub fn stop_times(&self) -> &Vec<RrStopTime> {
        &self.stop_times
    }

    pub fn route_stops(&self) -> &Vec<usize> {
        &self.route_stops
    }

    pub fn stops(&self) -> &Vec<RrStop> {
        &self.stops
    }

    pub fn stop_routes(&self) -> &Vec<usize> {
        &self.stop_routes
    }

    pub fn transfers(&self) -> &Vec<RrTransfer> {
        &self.transfers
    }
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
