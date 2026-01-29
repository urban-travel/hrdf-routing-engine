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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_route(arrival_time: &str, stop_id: i32) -> Route {
        let datetime_str = format!("2025-04-10 {}", arrival_time);
        let arrival_at = NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H:%M")
            .expect("Failed to parse datetime");

        let section = RouteSection::new(Some(1), stop_id - 1, stop_id, arrival_at, Some(300));
        let mut visited_stops = FxHashSet::default();
        visited_stops.insert(stop_id - 1);
        visited_stops.insert(stop_id);

        Route::new(vec![section], visited_stops)
    }

    #[test]
    fn test_route_queue_new() {
        let queue = RouteQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_route_queue_push_pop() {
        let mut queue = RouteQueue::new();
        let route = create_test_route("10:00", 1);

        queue.push(route.clone());
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);

        let popped = queue.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap(), route);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_route_queue_pop_empty() {
        let mut queue = RouteQueue::new();
        assert!(queue.pop().is_none());
    }

    #[test]
    fn test_route_queue_priority_ordering() {
        let mut queue = RouteQueue::new();

        // Push routes in non-sorted order
        let route_15_00 = create_test_route("15:00", 1);
        let route_10_00 = create_test_route("10:00", 2);
        let route_12_30 = create_test_route("12:30", 3);
        let route_08_00 = create_test_route("08:00", 4);

        queue.push(route_15_00.clone());
        queue.push(route_10_00.clone());
        queue.push(route_12_30.clone());
        queue.push(route_08_00.clone());

        assert_eq!(queue.len(), 4);

        // Pop routes - should come out in arrival time order (earliest first)
        let popped1 = queue.pop().unwrap();
        assert_eq!(popped1.arrival_at(), route_08_00.arrival_at());

        let popped2 = queue.pop().unwrap();
        assert_eq!(popped2.arrival_at(), route_10_00.arrival_at());

        let popped3 = queue.pop().unwrap();
        assert_eq!(popped3.arrival_at(), route_12_30.arrival_at());

        let popped4 = queue.pop().unwrap();
        assert_eq!(popped4.arrival_at(), route_15_00.arrival_at());

        assert!(queue.is_empty());
    }

    #[test]
    fn test_route_queue_fifo_for_same_arrival_time() {
        let mut queue = RouteQueue::new();

        // Push multiple routes with the same arrival time
        let route1 = create_test_route("10:00", 1);
        let route2 = create_test_route("10:00", 2);
        let route3 = create_test_route("10:00", 3);

        queue.push(route1.clone());
        queue.push(route2.clone());
        queue.push(route3.clone());

        assert_eq!(queue.len(), 3);

        // Routes with same arrival time should be popped in FIFO order (seq maintains order)
        let popped1 = queue.pop().unwrap();
        assert_eq!(popped1.arrival_stop_id(), route1.arrival_stop_id());

        let popped2 = queue.pop().unwrap();
        assert_eq!(popped2.arrival_stop_id(), route2.arrival_stop_id());

        let popped3 = queue.pop().unwrap();
        assert_eq!(popped3.arrival_stop_id(), route3.arrival_stop_id());

        assert!(queue.is_empty());
    }

    #[test]
    fn test_route_queue_len() {
        let mut queue = RouteQueue::new();
        assert_eq!(queue.len(), 0);

        queue.push(create_test_route("10:00", 1));
        assert_eq!(queue.len(), 1);

        queue.push(create_test_route("11:00", 2));
        assert_eq!(queue.len(), 2);

        queue.push(create_test_route("12:00", 3));
        assert_eq!(queue.len(), 3);

        queue.pop();
        assert_eq!(queue.len(), 2);

        queue.pop();
        assert_eq!(queue.len(), 1);

        queue.pop();
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_route_queue_is_empty() {
        let mut queue = RouteQueue::new();
        assert!(queue.is_empty());

        queue.push(create_test_route("10:00", 1));
        assert!(!queue.is_empty());

        queue.pop();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_route_queue_iter_routes() {
        let mut queue = RouteQueue::new();

        let route1 = create_test_route("10:00", 1);
        let route2 = create_test_route("11:00", 2);
        let route3 = create_test_route("12:00", 3);

        queue.push(route1.clone());
        queue.push(route2.clone());
        queue.push(route3.clone());

        let routes: Vec<&Route> = queue.iter_routes().collect();
        assert_eq!(routes.len(), 3);

        // Check that all routes are present (order may vary due to heap structure)
        let arrival_times: Vec<NaiveDateTime> = routes.iter().map(|r| r.arrival_at()).collect();
        assert!(arrival_times.contains(&route1.arrival_at()));
        assert!(arrival_times.contains(&route2.arrival_at()));
        assert!(arrival_times.contains(&route3.arrival_at()));
    }

    #[test]
    fn test_route_queue_iter_routes_empty() {
        let queue = RouteQueue::new();
        let routes: Vec<&Route> = queue.iter_routes().collect();
        assert_eq!(routes.len(), 0);
    }

    #[test]
    fn test_route_queue_mixed_operations() {
        let mut queue = RouteQueue::new();

        // Push some routes
        queue.push(create_test_route("15:00", 1));
        queue.push(create_test_route("10:00", 2));
        assert_eq!(queue.len(), 2);

        // Pop one (should get the 10:00 route)
        let popped = queue.pop().unwrap();
        assert_eq!(popped.arrival_at().format("%H:%M").to_string(), "10:00");
        assert_eq!(queue.len(), 1);

        // Push more routes
        queue.push(create_test_route("12:00", 3));
        queue.push(create_test_route("08:00", 4));
        assert_eq!(queue.len(), 3);

        // Pop all remaining routes in order
        let popped2 = queue.pop().unwrap();
        assert_eq!(popped2.arrival_at().format("%H:%M").to_string(), "08:00");

        let popped3 = queue.pop().unwrap();
        assert_eq!(popped3.arrival_at().format("%H:%M").to_string(), "12:00");

        let popped4 = queue.pop().unwrap();
        assert_eq!(popped4.arrival_at().format("%H:%M").to_string(), "15:00");

        assert!(queue.is_empty());
    }

    #[test]
    fn test_route_queue_seq_counter_increments() {
        let mut queue = RouteQueue::new();

        // Push routes with same arrival time to verify seq increments
        queue.push(create_test_route("10:00", 1));
        queue.push(create_test_route("10:00", 2));
        queue.push(create_test_route("10:00", 3));

        // The sequence counter should have incremented
        assert_eq!(queue.seq, 3);

        // Pop all and push more
        queue.pop();
        queue.pop();
        queue.pop();

        queue.push(create_test_route("11:00", 4));
        // Seq should continue incrementing
        assert_eq!(queue.seq, 4);
    }
}
