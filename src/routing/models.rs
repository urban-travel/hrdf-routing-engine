use chrono::{NaiveDateTime, NaiveTime};
use hrdf_parser::{DataStorage, Trip};

#[derive(Debug)]
pub struct RrRoute {
    stop_time_first_index: usize,
    stop_time_count: usize,
    stop_first_index: usize,
    stop_count: usize,
}

impl RrRoute {
    pub fn new(
        stop_time_first_index: usize,
        stop_time_count: usize,
        stop_first_index: usize,
        stop_count: usize,
    ) -> Self {
        Self {
            stop_time_first_index,
            stop_time_count,
            stop_first_index,
            stop_count,
        }
    }

    // Getters/Setters

    pub fn stop_time_first_index(&self) -> usize {
        self.stop_time_first_index
    }

    pub fn stop_time_count(&self) -> usize {
        self.stop_time_count
    }

    pub fn stop_first_index(&self) -> usize {
        self.stop_first_index
    }

    pub fn stop_count(&self) -> usize {
        self.stop_count
    }

    // Functions
}

#[derive(Debug)]
pub struct RrStopTime {
    arrival_time: Option<NaiveTime>,
    departure_time: Option<NaiveTime>,
}

impl RrStopTime {
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

#[derive(Debug)]
pub struct RrStop {
    id: i32,
    route_first_index: usize,
    route_count: usize,
}

impl RrStop {
    pub fn new(id: i32, route_first_index: usize, route_count: usize) -> Self {
        Self {
            id,
            route_first_index,
            route_count,
        }
    }

    // Getters/Setters

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn route_first_index(&self) -> usize {
        self.route_first_index
    }

    pub fn route_count(&self) -> usize {
        self.route_count
    }

    // Functions
}

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
