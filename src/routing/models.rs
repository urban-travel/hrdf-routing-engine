use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use hrdf_parser::{DataStorage, Trip};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::routing::RoutingData;

// ------------------------------------------------------------------------------------------------
// --- RrRoute
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RrRoute {
    trips: Vec<RrTrip>,
    stops: Vec<usize>,
    local_stop_index_by_stop_index: FxHashMap<usize, usize>,
}

impl RrRoute {
    pub fn new(trips: Vec<RrTrip>, stops: Vec<usize>) -> Self {
        Self {
            trips,
            stops,
            local_stop_index_by_stop_index: FxHashMap::default(),
        }
    }

    // Getters/Setters

    pub fn trips(&self) -> &Vec<RrTrip> {
        &self.trips
    }

    pub fn stops(&self) -> &Vec<usize> {
        &self.stops
    }

    pub fn set_stops(&mut self, value: Vec<usize>) {
        self.stops = value
    }

    pub fn local_stop_index_by_stop_index(&self) -> &FxHashMap<usize, usize> {
        &self.local_stop_index_by_stop_index
    }

    pub fn set_local_stop_index_by_stop_index(&mut self, value: FxHashMap<usize, usize>) {
        self.local_stop_index_by_stop_index = value;
    }

    // Functions

    pub fn arrival_time(&self, trip_index: usize, stop_index: usize) -> Option<NaiveTime> {
        self.trips[trip_index].schedule()[stop_index].arrival_time()
    }

    pub fn departure_time(&self, trip_index: usize, stop_index: usize) -> Option<NaiveTime> {
        self.trips[trip_index].schedule()[stop_index].departure_time()
    }
}

// ------------------------------------------------------------------------------------------------
// --- RrTrip
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RrTrip {
    id: i32,
    schedule: Vec<RrScheduleEntry>,
}

impl RrTrip {
    pub fn new(id: i32, schedule: Vec<RrScheduleEntry>) -> Self {
        Self { id, schedule }
    }

    // Getters/Setters

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn schedule(&self) -> &Vec<RrScheduleEntry> {
        &self.schedule
    }
}

// ------------------------------------------------------------------------------------------------
// --- RrScheduleEntry
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RrScheduleEntry {
    arrival_time: Option<NaiveTime>,
    departure_time: Option<NaiveTime>,
}

impl RrScheduleEntry {
    pub fn new(arrival_time: Option<NaiveTime>, departure_time: Option<NaiveTime>) -> Self {
        Self {
            arrival_time,
            departure_time,
        }
    }

    // Getters/Setters

    pub fn arrival_time(&self) -> Option<NaiveTime> {
        self.arrival_time
    }

    pub fn departure_time(&self) -> Option<NaiveTime> {
        self.departure_time
    }

    // Functions
}

// ------------------------------------------------------------------------------------------------
// --- RrStop
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RrStop {
    id: i32,
    routes: Vec<usize>,
    transfers: Vec<RrTransfer>,
}

impl RrStop {
    pub fn new(id: i32, routes: Vec<usize>) -> Self {
        Self {
            id,
            routes,
            transfers: Vec::new(),
        }
    }

    // Getters/Setters

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn routes(&self) -> &Vec<usize> {
        &self.routes
    }

    pub fn transfers(&self) -> &Vec<RrTransfer> {
        &self.transfers
    }

    pub fn set_transfers(&mut self, value: Vec<RrTransfer>) {
        self.transfers = value;
    }
}

// ------------------------------------------------------------------------------------------------
// --- RrTransfer
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RrTransfer {
    other_stop_index: usize,
    duration: TimeDelta,
}

impl RrTransfer {
    pub fn new(other_stop_index: usize, duration: TimeDelta) -> Self {
        Self {
            other_stop_index,
            duration,
        }
    }

    // Getters/Setters

    pub fn other_stop_index(&self) -> usize {
        self.other_stop_index
    }

    pub fn duration(&self) -> TimeDelta {
        self.duration
    }
}

// ------------------------------------------------------------------------------------------------
// --- AlgorithmArgs
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct AlgorithmArgs<'a> {
    routing_data: &'a RoutingData<'a>,
    departure_stop_index: usize,
    arrival_stop_index: usize,
    departure_at: NaiveDateTime,
    verbose: bool,
}

impl<'a> AlgorithmArgs<'a> {
    pub fn new(
        routing_data: &'a RoutingData<'a>,
        departure_stop_id: i32,
        arrival_stop_id: i32,
        departure_at: NaiveDateTime,
        verbose: bool,
    ) -> Self {
        let departure_stop_index = routing_data
            .stops()
            .iter()
            .position(|s| s.id() == departure_stop_id)
            .unwrap();
        let arrival_stop_index = routing_data
            .stops()
            .iter()
            .position(|s| s.id() == arrival_stop_id)
            .unwrap();

        Self {
            routing_data,
            departure_stop_index,
            arrival_stop_index,
            departure_at,
            verbose,
        }
    }

    // Getters/Setters

    pub fn routing_data(&self) -> &'a RoutingData<'a> {
        self.routing_data
    }

    pub fn departure_stop_index(&self) -> usize {
        self.departure_stop_index
    }

    pub fn arrival_stop_index(&self) -> usize {
        self.arrival_stop_index
    }

    pub fn departure_at(&self) -> NaiveDateTime {
        self.departure_at
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }
}

// ------------------------------------------------------------------------------------------------
// --- AlgorithmState
// ------------------------------------------------------------------------------------------------

pub type StopIndex = usize;
// pub type TripIndex = usize;

#[derive(Debug)]
pub struct AlgorithmState {
    labels: Vec<FxHashMap<StopIndex, NaiveTime>>,
    earliest_arrival_times: FxHashMap<StopIndex, NaiveTime>,
    marked_stops: FxHashSet<StopIndex>,
    predecessors: Vec<FxHashMap<usize, (i32, StopIndex)>>,
    current_round: usize,
}

impl AlgorithmState {
    pub fn new(algorithm_args: &AlgorithmArgs) -> Self {
        let mut labels = vec![FxHashMap::default()];
        labels[0].insert(
            algorithm_args.departure_stop_index(),
            algorithm_args.departure_at().time(),
        );

        let mut marked_stops = FxHashSet::default();
        marked_stops.insert(algorithm_args.departure_stop_index());

        Self {
            labels,
            earliest_arrival_times: FxHashMap::default(),
            marked_stops,
            predecessors: vec![FxHashMap::default()],
            current_round: 1,
        }
    }

    // Getters/Setters

    pub fn labels(&self) -> &Vec<FxHashMap<StopIndex, NaiveTime>> {
        &self.labels
    }

    pub fn labels_mut(&mut self) -> &mut Vec<FxHashMap<StopIndex, NaiveTime>> {
        &mut self.labels
    }

    pub fn earliest_arrival_times(&self) -> &FxHashMap<StopIndex, NaiveTime> {
        &self.earliest_arrival_times
    }

    pub fn earliest_arrival_times_mut(&mut self) -> &mut FxHashMap<StopIndex, NaiveTime> {
        &mut self.earliest_arrival_times
    }

    pub fn marked_stops(&self) -> &FxHashSet<StopIndex> {
        &self.marked_stops
    }

    pub fn marked_stops_mut(&mut self) -> &mut FxHashSet<StopIndex> {
        &mut self.marked_stops
    }

    pub fn predecessors(&self) -> &Vec<FxHashMap<usize, (i32, StopIndex)>> {
        &self.predecessors
    }

    pub fn predecessors_mut(&mut self) -> &mut Vec<FxHashMap<usize, (i32, StopIndex)>> {
        &mut self.predecessors
    }

    pub fn current_round(&self) -> usize {
        self.current_round
    }

    // Functions

    pub fn label(&self, stop_index: StopIndex) -> Option<NaiveTime> {
        self.labels()[self.current_round()]
            .get(&stop_index)
            .cloned()
    }

    pub fn previous_label(&self, stop_index: StopIndex) -> Option<NaiveTime> {
        self.labels()[self.current_round() - 1]
            .get(&stop_index)
            .cloned()
    }

    pub fn set_label(&mut self, stop_index: StopIndex, arrival_time: NaiveTime) {
        let k = self.current_round();
        self.labels_mut()[k].insert(stop_index, arrival_time);
    }

    pub fn earliest_arrival_time(&self, stop_index: StopIndex) -> Option<NaiveTime> {
        self.earliest_arrival_times().get(&stop_index).cloned()
    }

    pub fn set_earliest_arrival_time(&mut self, stop_index: StopIndex, arrival_time: NaiveTime) {
        self.earliest_arrival_times_mut()
            .insert(stop_index, arrival_time);
    }

    pub fn set_predecessor(
        &mut self,
        stop_index: StopIndex,
        trip_id: i32,
        trip_boarded_at_stop_index: StopIndex,
    ) {
        let k = self.current_round();
        self.predecessors_mut()[k - 1].insert(stop_index, (trip_id, trip_boarded_at_stop_index));
    }

    pub fn mark_stop(&mut self, stop_index: StopIndex) {
        self.marked_stops_mut().insert(stop_index);
    }

    pub fn next_round(&mut self) {
        self.current_round += 1
    }
}

// ------------------------------------------------------------------------------------------------
// --- Journey
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Journey {
    departure_at: NaiveDateTime,
    arrival_at: NaiveDateTime,
    legs: Vec<Leg>,
}

impl Journey {
    pub fn new(departure_at: NaiveDateTime, arrival_at: NaiveDateTime, legs: Vec<Leg>) -> Self {
        Self {
            departure_at,
            arrival_at,
            legs,
        }
    }

    // Getters/Setters

    pub fn legs(&self) -> &Vec<Leg> {
        &self.legs
    }
}

#[derive(Debug)]
pub struct Leg {
    trip_id: Option<i32>,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    departure_at: Option<NaiveDateTime>,
    arrival_at: Option<NaiveDateTime>,
    duration: Option<i16>,
}

impl Leg {
    pub fn new(
        trip_id: Option<i32>,
        departure_stop_id: i32,
        departure_at: Option<NaiveDateTime>,
        arrival_stop_id: i32,
        arrival_at: Option<NaiveDateTime>,
        duration: Option<i16>,
    ) -> Self {
        Self {
            trip_id,
            departure_stop_id,
            departure_at,
            arrival_stop_id,
            arrival_at,
            duration,
        }
    }

    // Getters/Setters

    pub fn departure_stop_id(&self) -> i32 {
        self.departure_stop_id
    }

    pub fn departure_at(&self) -> Option<NaiveDateTime> {
        self.departure_at
    }

    pub fn arrival_stop_id(&self) -> i32 {
        self.arrival_stop_id
    }

    pub fn arrival_at(&self) -> Option<NaiveDateTime> {
        self.arrival_at
    }

    pub fn duration(&self) -> Option<i16> {
        self.duration
    }

    // Functions

    pub fn trip<'a>(&'a self, data_storage: &'a DataStorage) -> Option<&'a Trip> {
        self.trip_id.map(|id| data_storage.trips().find(id))
    }

    pub fn is_transfer(&self) -> bool {
        self.trip_id.is_none()
    }
}
