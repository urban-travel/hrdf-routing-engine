use chrono::{Duration, NaiveDateTime, TimeDelta};
use hrdf_parser::{Coordinates, DataStorage, Journey, TransportType};
use rustc_hash::FxHashSet;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct RouteSection {
    journey_id: Option<i32>,
    departure_stop_id: i32,
    arrival_stop_id: i32,
    arrival_at: NaiveDateTime,
    duration: Option<i16>,
}

impl RouteSection {
    pub fn new(
        journey_id: Option<i32>,
        departure_stop_id: i32,
        arrival_stop_id: i32,
        arrival_at: NaiveDateTime,
        duration: Option<i16>,
    ) -> Self {
        Self {
            journey_id,
            departure_stop_id,
            arrival_stop_id,
            arrival_at,
            duration,
        }
    }

    // Getters/Setters

    pub fn journey_id(&self) -> Option<i32> {
        self.journey_id
    }

    pub fn departure_stop_id(&self) -> i32 {
        self.departure_stop_id
    }

    pub fn arrival_stop_id(&self) -> i32 {
        self.arrival_stop_id
    }

    pub fn set_arrival_stop_id(&mut self, value: i32) {
        self.arrival_stop_id = value;
    }

    pub fn arrival_at(&self) -> NaiveDateTime {
        self.arrival_at
    }

    pub fn set_arrival_at(&mut self, value: NaiveDateTime) {
        self.arrival_at = value;
    }

    pub fn duration(&self) -> Option<i16> {
        self.duration
    }

    // Functions

    // pub fn journey<'a>(&'a self, data_storage: &'a DataStorage) -> Option<&Journey> {
    //     self.journey_id.map(|id| data_storage.journeys().find(id))?
    pub fn journey<'a>(&'a self, data_storage: &'a DataStorage) -> Option<&'a Journey> {
        self.journey_id.map(|id| {
            data_storage
                .journeys()
                .find(id)
                .unwrap_or_else(|| panic!("Journey {:?} not found.", id))
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    sections: Vec<RouteSection>,
    visited_stops: FxHashSet<i32>,
}

impl Route {
    pub fn new(sections: Vec<RouteSection>, visited_stops: FxHashSet<i32>) -> Self {
        Self {
            sections,
            visited_stops,
        }
    }

    // Getters/Setters

    pub fn sections(&self) -> &Vec<RouteSection> {
        &self.sections
    }

    pub fn visited_stops(&self) -> &FxHashSet<i32> {
        &self.visited_stops
    }

    // Functions

    pub fn last_section(&self) -> &RouteSection {
        // A route always contains at least one section.
        self.sections.last().unwrap()
    }

    pub fn last_section_mut(&mut self) -> &mut RouteSection {
        // A route always contains at least one section.
        self.sections.last_mut().unwrap()
    }

    pub fn arrival_stop_id(&self) -> i32 {
        self.last_section().arrival_stop_id()
    }

    pub fn arrival_at(&self) -> NaiveDateTime {
        self.last_section().arrival_at()
    }

    pub fn has_visited_any_stops(&self, stops: &FxHashSet<i32>) -> bool {
        !self.visited_stops.is_disjoint(stops)
    }

    pub fn sections_having_journey(&self) -> Vec<&RouteSection> {
        self.sections
            .iter()
            .filter(|section| section.journey_id().is_some())
            .collect()
    }

    pub fn count_connections(&self) -> usize {
        self.sections_having_journey().len()
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum RoutingAlgorithmMode {
    SolveFromDepartureStopToArrivalStop,
    SolveFromDepartureStopToReachableArrivalStops,
}

pub struct RoutingAlgorithmArgs {
    mode: RoutingAlgorithmMode,
    arrival_stop_id: Option<i32>,
    time_limit: Option<NaiveDateTime>,
}

impl RoutingAlgorithmArgs {
    pub fn new(
        mode: RoutingAlgorithmMode,
        arrival_stop_id: Option<i32>,
        time_limit: Option<NaiveDateTime>,
    ) -> Self {
        Self {
            mode,
            arrival_stop_id,
            time_limit,
        }
    }

    pub fn solve_from_departure_stop_to_arrival_stop(arrival_stop_id: i32) -> Self {
        Self::new(
            RoutingAlgorithmMode::SolveFromDepartureStopToArrivalStop,
            Some(arrival_stop_id),
            None,
        )
    }

    pub fn solve_from_departure_stop_to_reachable_arrival_stops(time_limit: NaiveDateTime) -> Self {
        Self::new(
            RoutingAlgorithmMode::SolveFromDepartureStopToReachableArrivalStops,
            None,
            Some(time_limit),
        )
    }

    // Getters/Setters

    pub fn mode(&self) -> RoutingAlgorithmMode {
        self.mode
    }

    /// Do not call this function if you are not sure that arrival_stop_id is not None.
    pub fn arrival_stop_id(&self) -> i32 {
        self.arrival_stop_id.unwrap()
    }

    /// Do not call this function if you are not sure that time_limit is not None.
    pub fn time_limit(&self) -> NaiveDateTime {
        self.time_limit.unwrap()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteResult {
    departure_at: NaiveDateTime,
    arrival_at: NaiveDateTime,
    sections: Vec<RouteSectionResult>,
}

// impl Clone for RouteResult {
//     fn clone(&self) -> Self {
//         RouteResult {
//             departure_at: self.departure_at,
//             arrival_at: self.arrival_at,
//             sections: self.sections.clone(),
//         }
//     }
// }

impl RouteResult {
    pub fn new(
        departure_at: NaiveDateTime,
        arrival_at: NaiveDateTime,
        sections: Vec<RouteSectionResult>,
    ) -> Self {
        Self {
            departure_at,
            arrival_at,
            sections,
        }
    }

    // Getters/Setters

    pub fn departure_at(&self) -> NaiveDateTime {
        if let Some(rs) = self.sections.first() {
            if rs.is_walking_trip() {
                self.departure_at - TimeDelta::minutes(rs.duration().unwrap() as i64)
            } else {
                self.departure_at
            }
        } else {
            self.departure_at
        }
    }

    pub fn arrival_at(&self) -> NaiveDateTime {
        if let Some(rs) = self.sections.last() {
            if rs.is_walking_trip() {
                self.arrival_at + TimeDelta::minutes(rs.duration().unwrap() as i64)
            } else {
                self.arrival_at
            }
        } else {
            self.arrival_at
        }
    }

    pub fn sections(&self) -> &Vec<RouteSectionResult> {
        &self.sections
    }

    pub fn number_changes(&self) -> usize {
        if !self.sections().is_empty() {
            self.sections()
                .iter()
                .filter(|s| !s.is_walking_trip())
                .count()
                - 1
        } else {
            0
        }
    }

    pub fn total_walking_time(&self) -> Duration {
        self.sections()
            .iter()
            .filter(|s| s.is_walking_trip())
            .fold(Duration::minutes(0), |total, d| {
                total + Duration::minutes(d.duration().unwrap_or(0i16) as i64)
            })
    }

    pub fn total_time(&self) -> Duration {
        self.arrival_at() - self.departure_at()
    }

    pub fn departure_stop_id(&self) -> Option<i32> {
        self.sections().first().map(|s| s.departure_stop_id)
    }

    pub fn arrival_stop_id(&self) -> Option<i32> {
        self.sections().last().map(|s| s.arrival_stop_id())
    }

    pub fn departure_stop_name(&self, data_storage: &DataStorage) -> Option<String> {
        self.departure_stop_id().map(|id| {
            String::from(
                data_storage
                    .stops()
                    .find(id)
                    .unwrap_or_else(|| panic!("stop {id} not found"))
                    .name(),
            )
        })
    }

    pub fn arrival_stop_name(&self, data_storage: &DataStorage) -> Option<String> {
        self.arrival_stop_id().map(|id| {
            String::from(
                data_storage
                    .stops()
                    .find(id)
                    .unwrap_or_else(|| panic!("stop {id} not found"))
                    .name(),
            )
        })
    }
}

#[derive(Debug, Serialize, Copy, Clone)]
pub struct RouteSectionResult {
    journey_id: Option<i32>,
    departure_stop_id: i32,
    departure_stop_lv95_coordinates: Option<Coordinates>,
    departure_stop_wgs84_coordinates: Option<Coordinates>,
    arrival_stop_id: i32,
    arrival_stop_lv95_coordinates: Option<Coordinates>,
    arrival_stop_wgs84_coordinates: Option<Coordinates>,
    departure_at: Option<NaiveDateTime>,
    arrival_at: Option<NaiveDateTime>,
    duration: Option<i16>,
    transport: Transport,
}

impl RouteSectionResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        journey_id: Option<i32>,
        departure_stop_id: i32,
        departure_stop_lv95_coordinates: Option<Coordinates>,
        departure_stop_wgs84_coordinates: Option<Coordinates>,
        arrival_stop_id: i32,
        arrival_stop_lv95_coordinates: Option<Coordinates>,
        arrival_stop_wgs84_coordinates: Option<Coordinates>,
        departure_at: Option<NaiveDateTime>,
        arrival_at: Option<NaiveDateTime>,
        duration: Option<i16>,
        transport: Transport,
    ) -> Self {
        Self {
            journey_id,
            departure_stop_id,
            departure_stop_lv95_coordinates,
            departure_stop_wgs84_coordinates,
            arrival_stop_id,
            arrival_stop_lv95_coordinates,
            arrival_stop_wgs84_coordinates,
            departure_at,
            arrival_at,
            duration,
            transport,
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

    pub fn arrival_stop_lv95_coordinates(&self) -> Option<Coordinates> {
        self.arrival_stop_lv95_coordinates
    }

    // pub fn arrival_stop_wgs84_coordinates(&self) -> Option<Coordinates> {
    //     self.arrival_stop_wgs84_coordinates
    // }

    pub fn arrival_at(&self) -> Option<NaiveDateTime> {
        self.arrival_at
    }

    pub fn duration(&self) -> Option<i16> {
        self.duration
    }

    // Functions
    pub fn journey<'a>(&'a self, data_storage: &'a DataStorage) -> Option<&'a Journey> {
        self.journey_id.map(|id| {
            data_storage
                .journeys()
                .find(id)
                .unwrap_or_else(|| panic!("Journey {:?} not found.", id))
        })
    }

    pub fn departure_stop_name<'a>(&'a self, data_storage: &'a DataStorage) -> &'a str {
        let id = self.departure_stop_id();
        data_storage
            .stops()
            .find(id)
            .unwrap_or_else(|| panic!("stop {id} not found"))
            .name()
    }

    pub fn arrival_stop_name<'a>(&'a self, data_storage: &'a DataStorage) -> &'a str {
        let id = self.arrival_stop_id();
        data_storage
            .stops()
            .find(id)
            .unwrap_or_else(|| panic!("stop {id} not found"))
            .name()
    }

    pub fn is_walking_trip(&self) -> bool {
        self.journey_id.is_none()
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum Transport {
    Boat,
    Bus,
    Chairlift,
    Elevator,
    Funicular,
    GondolaLift,
    RackRailroad,
    Train,
    Tramway,
    Unknown,
    Underground,
    Walk,
}

impl From<&TransportType> for Transport {
    fn from(value: &TransportType) -> Self {
        match value.designation() {
            "SL" => Transport::Chairlift,
            "ASC" => Transport::Elevator,
            "CC" => Transport::RackRailroad,
            "BAT" | "FAE" => Transport::Boat,
            "B" | "BN" | "BP" | "CAR" | "EV" | "EXB" | "RUB" | "TX" => Transport::Bus,
            "FUN" => Transport::Funicular,
            "M" => Transport::Underground,
            "GB" | "PB" => Transport::GondolaLift,
            "EC" | "EXT" | "IC" | "ICE" | "IR" | "NJ" | "PE" | "R" | "RB" | "RE" | "RJX" | "S"
            | "TER" | "TGV" | "SN" => Transport::Train,
            "T" => Transport::Tramway,
            "UUU" => Transport::Unknown,
            _ => panic!("Uknown transport designation: {}", value.designation()),
        }
    }
}
