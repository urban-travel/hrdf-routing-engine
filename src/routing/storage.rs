// ------------------------------------------------------------------------------------------------
// --- RrStorage
// ------------------------------------------------------------------------------------------------

use chrono::Duration;
use hrdf_parser::{DataStorage, Model};
use rustc_hash::FxHashMap;

use crate::routing::models::{RrRoute, RrScheduleEntry, RrStop, RrTransfer, RrTrip};

#[derive(Debug)]
pub struct RoutingData<'a> {
    data_storage: &'a DataStorage,
    routes: Vec<RrRoute>,
    stops: Vec<RrStop>,
}

impl<'a> RoutingData<'a> {
    pub fn new(data_storage: &'a DataStorage) -> Self {
        let mut routes = get_routes(data_storage);
        let stops = get_stops(data_storage, &routes);
        fix_route_stops(&mut routes, &stops);

        for route in &mut routes {
            route.set_local_stop_index_by_stop_index(route.stops().iter().enumerate().fold(
                FxHashMap::default(),
                |mut acc, (i, &stop_index)| {
                    acc.insert(stop_index, i);
                    acc
                },
            ));
        }

        Self {
            data_storage,
            routes,
            stops,
        }
    }

    // Getters/Setters

    pub fn data_storage(&self) -> &DataStorage {
        &self.data_storage
    }

    pub fn routes(&self) -> &Vec<RrRoute> {
        &self.routes
    }

    pub fn stops(&self) -> &Vec<RrStop> {
        &self.stops
    }
}

fn get_routes(data_storage: &DataStorage) -> Vec<RrRoute> {
    let mut tmp_routes = FxHashMap::default();

    for trip in data_storage.trips().entries() {
        let route_id = trip.hash_route().unwrap();

        if !tmp_routes.contains_key(&route_id) {
            tmp_routes.insert(route_id, Vec::new());
        }

        tmp_routes.get_mut(&route_id).unwrap().push(trip);
    }

    let mut routes = Vec::new();

    for mut trips in tmp_routes.into_values() {
        trips.sort_by(|a, b| {
            let a = a.route().first().unwrap().departure_time();
            let b = b.route().first().unwrap().departure_time();
            a.cmp(b)
        });

        let mut route_trips = Vec::new();

        for trip in &trips {
            let mut schedule = Vec::new();

            for route_entry in trip.route() {
                schedule.push(RrScheduleEntry::new(
                    *route_entry.arrival_time(),
                    *route_entry.departure_time(),
                ));
            }

            route_trips.push(RrTrip::new(trip.id(), schedule));
        }

        let mut route_stops = Vec::new();

        for route_entry in trips.first().unwrap().route() {
            route_stops.push(route_entry.stop_id() as usize);
        }

        routes.push(RrRoute::new(route_trips, route_stops));
    }

    routes
}

fn get_stops(data_storage: &DataStorage, routes: &Vec<RrRoute>) -> Vec<RrStop> {
    let mut tmp_stops = FxHashMap::default();

    for (i, route) in routes.iter().enumerate() {
        for &stop_id in route.stops() {
            let stop_id = stop_id as i32;

            if !tmp_stops.contains_key(&stop_id) {
                tmp_stops.insert(stop_id, Vec::new());
            }

            tmp_stops.get_mut(&stop_id).unwrap().push(i);
        }
    }

    let mut stops = Vec::new();

    for (stop_id, stop_routes) in tmp_stops {
        stops.push(RrStop::new(stop_id, stop_routes));
    }

    for i in 0..stops.len() {
        let stop_connections = data_storage
            .stop_connections_by_stop_id()
            .get(&stops[i].id());

        if stop_connections.is_none() {
            continue;
        }

        let mut transfers = Vec::new();

        for stop_connection_id in stop_connections.unwrap() {
            let stop_connection = data_storage.stop_connections().find(*stop_connection_id);
            let other_stop_index = stops
                .iter()
                .position(|s| s.id() == stop_connection.stop_id_2());

            if let Some(index) = other_stop_index {
                transfers.push(RrTransfer::new(
                    index,
                    Duration::minutes(stop_connection.duration() as i64),
                ));
            }
        }

        stops[i].set_transfers(transfers);
    }

    stops
}

fn fix_route_stops(routes: &mut Vec<RrRoute>, stops: &Vec<RrStop>) {
    for route in routes {
        route.set_stops(
            route
                .stops()
                .iter()
                .map(|&stop_id| stops.iter().position(|s| s.id() == stop_id as i32).unwrap())
                .collect(),
        );
    }
}
