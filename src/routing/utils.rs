use std::cmp::Ordering;
use std::collections::BinaryHeap;

use chrono::NaiveDateTime;
use hrdf_parser::{DataStorage, StopConnection};
use rustc_hash::FxHashSet;

use super::models::{Route, RouteSection};

#[derive(Debug)]
struct RouteHeapItem {
    arrival_at: NaiveDateTime,
    seq: u64,
    route: Route,
}

impl Eq for RouteHeapItem {}

impl PartialEq for RouteHeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.arrival_at == other.arrival_at && self.seq == other.seq
    }
}

impl Ord for RouteHeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.arrival_at.cmp(&self.arrival_at) {
            Ordering::Equal => other.seq.cmp(&self.seq),
            ordering => ordering,
        }
    }
}

impl PartialOrd for RouteHeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct RouteQueue {
    heap: BinaryHeap<RouteHeapItem>,
    seq: u64,
}

impl RouteQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            seq: 0,
        }
    }

    pub fn push(&mut self, route: Route) {
        self.heap.push(RouteHeapItem {
            arrival_at: route.arrival_at(),
            seq: self.seq,
            route,
        });
        self.seq += 1;
    }

    pub fn pop(&mut self) -> Option<Route> {
        self.heap.pop().map(|item| item.route)
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn iter_routes(&self) -> impl Iterator<Item = &Route> {
        self.heap.iter().map(|item| &item.route)
    }
}

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
