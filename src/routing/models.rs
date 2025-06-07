use std::hash::Hasher;

use chrono::{NaiveDateTime, NaiveTime};
use hrdf_parser::{DataStorage, Trip};
use rustc_hash::FxHashMap;

pub struct RrRoute {
    id: u64,
    trips: Vec<i32>,
    stops: Vec<i32>,
    trip_departure_times: FxHashMap<(i32, i32), NaiveTime>,
    trip_arrival_times: FxHashMap<(i32, i32), NaiveTime>,
}

impl RrRoute {
    pub fn new() -> Self {
        Self {
            id: 0,
            trips: Vec::new(),
            stops: Vec::new(),
            trip_departure_times: FxHashMap::default(),
            trip_arrival_times: FxHashMap::default(),
        }
    }

    // Getters/Setters

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn trips(&self) -> &Vec<i32> {
        &self.trips
    }

    pub fn trips_mut(&mut self) -> &mut Vec<i32> {
        &mut self.trips
    }

    pub fn stops(&self) -> &Vec<i32> {
        &self.stops
    }

    pub fn trip_departure_times_mut(&mut self) -> &mut FxHashMap<(i32, i32), NaiveTime> {
        &mut self.trip_departure_times
    }

    pub fn trip_arrival_times_mut(&mut self) -> &mut FxHashMap<(i32, i32), NaiveTime> {
        &mut self.trip_arrival_times
    }

    // Functions

    pub fn add_trip(&mut self, trip_id: i32) {
        self.trips.push(trip_id);
    }

    pub fn add_stop(&mut self, stop_id: i32) {
        self.stops.push(stop_id);
    }

    pub fn departure_time(&self, trip_id: i32, stop_id: i32) -> Option<&NaiveTime> {
        self.trip_departure_times.get(&(trip_id, stop_id))
    }

    pub fn arrival_time(&self, trip_id: i32, stop_id: i32) -> Option<&NaiveTime> {
        self.trip_arrival_times.get(&(trip_id, stop_id))
    }

    pub fn update_id(&mut self, data_storage: &DataStorage) {
        let trip = data_storage.trips().find(*self.trips().first().unwrap());
        self.id = trip.hash_route().unwrap();
    }
}

// pub struct RrStop<'a> {
//     routes: Vec<&'a RrRoute>,
// }

// ------------------------------------------------------------------------------------------------
// --- Result
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
