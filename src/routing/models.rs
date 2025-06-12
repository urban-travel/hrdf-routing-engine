use chrono::NaiveTime;
use rustc_hash::FxHashMap;

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
    transfer_first_index: usize,
    transfer_count: usize,
}

impl RrStop {
    pub fn new(id: i32, routes: Vec<usize>) -> Self {
        Self {
            id,
            routes,
            transfer_first_index: 0,
            transfer_count: 0,
        }
    }

    // Getters/Setters

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn routes(&self) -> &Vec<usize> {
        &self.routes
    }

    pub fn transfer_first_index(&self) -> usize {
        self.transfer_first_index
    }

    pub fn set_transfer_first_index(&mut self, value: usize) {
        self.transfer_first_index = value;
    }

    pub fn transfer_count(&self) -> usize {
        self.transfer_count
    }

    pub fn set_transfer_count(&mut self, value: usize) {
        self.transfer_count = value;
    }
}

// ------------------------------------------------------------------------------------------------
// --- RrTransfer
// ------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RrTransfer {
    other_stop_index: usize,
    duration: i16,
}

impl RrTransfer {
    pub fn new(other_stop_index: usize, duration: i16) -> Self {
        Self {
            other_stop_index,
            duration,
        }
    }

    // Getters/Setters

    pub fn other_stop_index(&self) -> usize {
        self.other_stop_index
    }

    pub fn duration(&self) -> i16 {
        self.duration
    }
}

// // ------------------------------------------------------------------------------------------------
// // --- Result
// // ------------------------------------------------------------------------------------------------

// #[derive(Debug)]
// pub struct Journey {
//     departure_at: NaiveDateTime,
//     arrival_at: NaiveDateTime,
//     legs: Vec<Leg>,
// }

// impl Journey {
//     pub fn new(departure_at: NaiveDateTime, arrival_at: NaiveDateTime, legs: Vec<Leg>) -> Self {
//         Self {
//             departure_at,
//             arrival_at,
//             legs,
//         }
//     }

//     // Getters/Setters

//     pub fn legs(&self) -> &Vec<Leg> {
//         &self.legs
//     }
// }

// #[derive(Debug)]
// pub struct Leg {
//     trip_id: Option<i32>,
//     departure_stop_id: i32,
//     arrival_stop_id: i32,
//     departure_at: Option<NaiveDateTime>,
//     arrival_at: Option<NaiveDateTime>,
//     duration: Option<i16>,
// }

// impl Leg {
//     pub fn new(
//         trip_id: Option<i32>,
//         departure_stop_id: i32,
//         departure_at: Option<NaiveDateTime>,
//         arrival_stop_id: i32,
//         arrival_at: Option<NaiveDateTime>,
//         duration: Option<i16>,
//     ) -> Self {
//         Self {
//             trip_id,
//             departure_stop_id,
//             departure_at,
//             arrival_stop_id,
//             arrival_at,
//             duration,
//         }
//     }

//     // Getters/Setters

//     pub fn departure_stop_id(&self) -> i32 {
//         self.departure_stop_id
//     }

//     pub fn departure_at(&self) -> Option<NaiveDateTime> {
//         self.departure_at
//     }

//     pub fn arrival_stop_id(&self) -> i32 {
//         self.arrival_stop_id
//     }

//     pub fn arrival_at(&self) -> Option<NaiveDateTime> {
//         self.arrival_at
//     }

//     pub fn duration(&self) -> Option<i16> {
//         self.duration
//     }

//     // Functions

//     pub fn trip<'a>(&'a self, data_storage: &'a DataStorage) -> Option<&'a Trip> {
//         self.trip_id.map(|id| data_storage.trips().find(id))
//     }

//     pub fn is_transfer(&self) -> bool {
//         self.trip_id.is_none()
//     }
// }
