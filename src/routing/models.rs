use chrono::{Duration, NaiveDateTime, TimeDelta};
use hrdf_parser::{Coordinates, DataStorage, Journey, TransportType};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    sections: Vec<RouteSection>,
    visited_stops: FxHashSet<i32>,
}

impl Hash for Route {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the sections vector
        self.sections.hash(state);

        // Hash the visited stops set
        // Note: FxHashSet does not guarantee order, so we should hash in a stable way
        // For consistency, iterate in sorted order
        let mut stops: Vec<_> = self.visited_stops.iter().collect();
        stops.sort();
        stops.hash(state);
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn create_test_section(
        journey_id: Option<i32>,
        dep_stop: i32,
        arr_stop: i32,
        dep_time: &str,
        arr_time: &str,
        duration: Option<i16>,
        transport: Transport,
    ) -> RouteSectionResult {
        let dep_at = if dep_time.is_empty() {
            None
        } else {
            Some(NaiveDateTime::parse_from_str(dep_time, "%Y-%m-%d %H:%M:%S").unwrap())
        };
        let arr_at = if arr_time.is_empty() {
            None
        } else {
            Some(NaiveDateTime::parse_from_str(arr_time, "%Y-%m-%d %H:%M:%S").unwrap())
        };

        RouteSectionResult::new(
            journey_id, dep_stop, None, None, // LV95 and WGS84 coordinates
            arr_stop, None, None, dep_at, arr_at, duration, transport,
        )
    }

    #[test]
    fn test_route_result_total_time() {
        let sections = vec![create_test_section(
            Some(1),
            8503000,
            8507000,
            "2025-06-15 10:00:00",
            "2025-06-15 11:30:00",
            None,
            Transport::Train,
        )];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.total_time().num_minutes(), 90);
    }

    #[test]
    fn test_route_result_number_changes_direct() {
        let sections = vec![create_test_section(
            Some(1),
            8503000,
            8507000,
            "2025-06-15 10:00:00",
            "2025-06-15 11:00:00",
            None,
            Transport::Train,
        )];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.number_changes(), 0, "Direct journey has no changes");
    }

    #[test]
    fn test_route_result_number_changes_with_one_transfer() {
        let sections = vec![
            create_test_section(
                Some(1),
                8503000,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 10:45:00",
                None,
                Transport::Train,
            ),
            create_test_section(
                Some(2),
                8507000,
                8508000,
                "2025-06-15 10:50:00",
                "2025-06-15 11:30:00",
                None,
                Transport::Train,
            ),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.number_changes(), 1, "One transfer = 1 change");
    }

    #[test]
    fn test_route_result_number_changes_with_two_transfers() {
        let sections = vec![
            create_test_section(
                Some(1),
                8503000,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 10:45:00",
                None,
                Transport::Train,
            ),
            create_test_section(
                Some(2),
                8507000,
                8508000,
                "2025-06-15 10:50:00",
                "2025-06-15 11:30:00",
                None,
                Transport::Train,
            ),
            create_test_section(
                Some(3),
                8508000,
                8509000,
                "2025-06-15 11:35:00",
                "2025-06-15 12:00:00",
                None,
                Transport::Train,
            ),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.number_changes(), 2, "Two transfers = 2 changes");
    }

    #[test]
    fn test_route_result_number_changes_excludes_walking() {
        let sections = vec![
            create_test_section(None, 8503000, 8503001, "", "", Some(5), Transport::Walk),
            create_test_section(
                Some(1),
                8503001,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 10:45:00",
                None,
                Transport::Train,
            ),
            create_test_section(
                Some(2),
                8507000,
                8508000,
                "2025-06-15 10:50:00",
                "2025-06-15 11:30:00",
                None,
                Transport::Train,
            ),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Walking doesn't count as a change, only the train-to-train transfer
        assert_eq!(route.number_changes(), 1);
    }

    #[test]
    fn test_route_result_total_walking_time() {
        let sections = vec![
            create_test_section(None, 8503000, 8503001, "", "", Some(5), Transport::Walk),
            create_test_section(
                Some(1),
                8503001,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 11:00:00",
                None,
                Transport::Train,
            ),
            create_test_section(None, 8507000, 8507001, "", "", Some(3), Transport::Walk),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(
            route.total_walking_time().num_minutes(),
            8,
            "5 + 3 = 8 minutes"
        );
    }

    #[test]
    fn test_route_result_total_walking_time_no_walking() {
        let sections = vec![create_test_section(
            Some(1),
            8503000,
            8507000,
            "2025-06-15 10:00:00",
            "2025-06-15 11:00:00",
            None,
            Transport::Train,
        )];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.total_walking_time().num_minutes(), 0);
    }

    #[test]
    fn test_route_result_total_walking_time_only_walking() {
        let sections = vec![
            create_test_section(None, 8503000, 8503001, "", "", Some(5), Transport::Walk),
            create_test_section(None, 8503001, 8503002, "", "", Some(7), Transport::Walk),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:12:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.total_walking_time().num_minutes(), 12);
    }

    #[test]
    fn test_route_result_departure_at_with_initial_walking() {
        let sections = vec![
            create_test_section(None, 8503000, 8503001, "", "", Some(5), Transport::Walk),
            create_test_section(
                Some(1),
                8503001,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 11:00:00",
                None,
                Transport::Train,
            ),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Should subtract 5 minutes of walking from departure time
        let expected =
            NaiveDateTime::parse_from_str("2025-06-15 09:55:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(route.departure_at(), expected);
    }

    #[test]
    fn test_route_result_departure_at_without_walking() {
        let sections = vec![create_test_section(
            Some(1),
            8503000,
            8507000,
            "2025-06-15 10:00:00",
            "2025-06-15 11:00:00",
            None,
            Transport::Train,
        )];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Should return the stored departure time
        assert_eq!(route.departure_at(), dep_at);
    }

    #[test]
    fn test_route_result_arrival_at_with_final_walking() {
        let sections = vec![
            create_test_section(
                Some(1),
                8503000,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 11:00:00",
                None,
                Transport::Train,
            ),
            create_test_section(None, 8507000, 8507001, "", "", Some(3), Transport::Walk),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Should add 3 minutes of walking to arrival time
        let expected =
            NaiveDateTime::parse_from_str("2025-06-15 11:03:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(route.arrival_at(), expected);
    }

    #[test]
    fn test_route_result_arrival_at_without_walking() {
        let sections = vec![create_test_section(
            Some(1),
            8503000,
            8507000,
            "2025-06-15 10:00:00",
            "2025-06-15 11:00:00",
            None,
            Transport::Train,
        )];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Should return the stored arrival time
        assert_eq!(route.arrival_at(), arr_at);
    }

    #[test]
    fn test_route_result_empty_sections() {
        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, dep_at, vec![]);

        assert_eq!(route.number_changes(), 0);
        assert_eq!(route.total_walking_time().num_minutes(), 0);
        assert_eq!(route.total_time().num_minutes(), 0);
        assert_eq!(route.departure_stop_id(), None);
        assert_eq!(route.arrival_stop_id(), None);
    }

    #[test]
    fn test_route_result_stop_ids() {
        let sections = vec![create_test_section(
            Some(1),
            8503000,
            8507000,
            "2025-06-15 10:00:00",
            "2025-06-15 11:00:00",
            None,
            Transport::Train,
        )];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        assert_eq!(route.departure_stop_id(), Some(8503000));
        assert_eq!(route.arrival_stop_id(), Some(8507000));
    }

    #[test]
    fn test_route_result_stop_ids_with_walking() {
        let sections = vec![
            create_test_section(None, 8503000, 8503001, "", "", Some(5), Transport::Walk),
            create_test_section(
                Some(1),
                8503001,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 11:00:00",
                None,
                Transport::Train,
            ),
            create_test_section(None, 8507000, 8507001, "", "", Some(3), Transport::Walk),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Should return first and last stops (including walking)
        assert_eq!(route.departure_stop_id(), Some(8503000));
        assert_eq!(route.arrival_stop_id(), Some(8507001));
    }

    #[test]
    fn test_route_result_total_time_with_walking() {
        let sections = vec![
            create_test_section(None, 8503000, 8503001, "", "", Some(5), Transport::Walk),
            create_test_section(
                Some(1),
                8503001,
                8507000,
                "2025-06-15 10:00:00",
                "2025-06-15 11:00:00",
                None,
                Transport::Train,
            ),
            create_test_section(None, 8507000, 8507001, "", "", Some(3), Transport::Walk),
        ];

        let dep_at =
            NaiveDateTime::parse_from_str("2025-06-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let arr_at =
            NaiveDateTime::parse_from_str("2025-06-15 11:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let route = RouteResult::new(dep_at, arr_at, sections);

        // Total time should account for walking: (11:03) - (09:55) = 68 minutes
        assert_eq!(route.total_time().num_minutes(), 68);
    }
}
