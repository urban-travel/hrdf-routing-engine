use chrono::NaiveDateTime;
use hrdf_parser::{DataStorage, Trip};

#[derive(Eq, Hash, PartialEq)]
pub struct RrRoute {
    trips: Vec<i32>,
}

impl RrRoute {
    pub fn new() -> Self {
        Self { trips: Vec::new() }
    }

    // Getters/Setters

    pub fn trips(&self) -> &Vec<i32> {
        &self.trips
    }

    pub fn trips_mut(&mut self) -> &mut Vec<i32> {
        &mut self.trips
    }

    // Functions

    pub fn add_trip(&mut self, trip_id: i32) {
        self.trips.push(trip_id);
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
