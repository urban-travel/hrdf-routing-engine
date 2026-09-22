use chrono::NaiveDate;
use hrdf_parser::{DataStorage, Journey, Model};
use rustc_hash::FxHashSet;

use crate::routing::models::Transport;

use super::{
    models::{Route, RouteResult, RouteSection, RouteSectionResult},
    utils::clone_update_route,
};

impl Route {
    pub fn extend(
        &self,
        data_storage: &DataStorage,
        journey_id: i32,
        date: NaiveDate,
        is_departure_date: bool,
    ) -> Option<Route> {
        let journey = data_storage
            .journeys()
            .find(journey_id)
            // .expect(format!("Jounrey {journey_id} not found").as_str());
            .unwrap_or_else(|| panic!("Journey {:?} not found.", journey_id));

        if journey
            .is_last_stop(self.arrival_stop_id(), false)
            .unwrap_or_else(|_| panic!("Unable to get last stop for {}", self.arrival_stop_id()))
        {
            return None;
        }

        let is_same_journey = self.last_section().journey_id() == Some(journey_id);

        RouteSection::find_next(
            data_storage,
            journey,
            self.arrival_stop_id(),
            date,
            is_departure_date,
        )
        .and_then(|(new_section, new_visited_stops)| {
            // When extending on the same journey, check intermediate stops for cycles
            // to prevent a journey from looping back on itself.
            // When transferring to a new journey, skip the cycle check — different journeys
            // legitimately share stops, and the earliest-arrival pruning already
            // prevents redundant exploration at exchange points.
            if is_same_journey
                && self.has_visited_any_stops(&new_visited_stops)
                && new_section.arrival_stop_id()
                    != journey
                        .first_stop_id()
                        .unwrap_or_else(|_| panic!("No first stop on {journey:?}"))
            {
                return None;
            }

            let new_route = clone_update_route(self, |cloned_sections, cloned_visited_stops| {
                if is_same_journey {
                    let last_section = cloned_sections.last_mut().unwrap();
                    last_section.set_arrival_stop_id(new_section.arrival_stop_id());
                    last_section.set_arrival_at(new_section.arrival_at());
                } else {
                    cloned_sections.push(new_section);
                }

                cloned_visited_stops.extend(new_visited_stops);
            });
            Some(new_route)
        })
    }

    pub fn to_route_result(&self, data_storage: &DataStorage) -> RouteResult {
        let sections: Vec<_> = self
            .sections()
            .iter()
            .map(|section| section.to_route_section_result(data_storage))
            .collect();

        let departure_at = if sections.first().unwrap().is_walking_trip() {
            // This section is guaranteed not to be a walking trip.
            sections[1].departure_at().unwrap()
        } else {
            sections.first().unwrap().departure_at().unwrap()
        };

        let arrival_at = if sections.last().unwrap().is_walking_trip() {
            // This section is guaranteed not to be a walking trip.
            sections[sections.len() - 2].arrival_at().unwrap()
        } else {
            sections.last().unwrap().arrival_at().unwrap()
        };

        RouteResult::new(departure_at, arrival_at, sections)
    }

    pub fn extend_reverse(
        &self,
        data_storage: &DataStorage,
        journey_id: i32,
        date: NaiveDate,
        is_arrival_date: bool,
    ) -> Option<Route> {
        let journey = data_storage
            .journeys()
            .find(journey_id)
            .unwrap_or_else(|| panic!("Journey {:?} not found.", journey_id));

        // Check if we are at the first stop of the journey (physically)
        // In reverse search, this means we cannot go further "back" on this journey.
        let is_first = journey
            .is_first_stop(self.arrival_stop_id(), false)
            .unwrap_or_else(|_| panic!("Unable to get first stop for {}", self.arrival_stop_id()));
        if is_first {
            return None;
        }

        let is_same_journey = self.last_section().journey_id() == Some(journey_id);

        RouteSection::find_previous(
            data_storage,
            journey,
            self.arrival_stop_id(),
            date,
            is_arrival_date,
        )
        .and_then(|(new_section, new_visited_stops)| {
            // When extending on the same journey, check intermediate stops for cycles
            // to prevent a journey from looping back on itself.
            // When transferring to a new journey, skip the cycle check — different journeys
            // legitimately share stops, and the connection-time pruning already
            // prevents redundant exploration at exchange points.
            if is_same_journey
                && self.has_visited_any_stops(&new_visited_stops)
                && new_section.arrival_stop_id()
                    != journey
                        .last_stop_id()
                        .unwrap_or_else(|_| panic!("No last stop on {journey:?}"))
            {
                return None;
            }

            let new_route = clone_update_route(self, |cloned_sections, cloned_visited_stops| {
                if is_same_journey {
                    let last_section = cloned_sections.last_mut().unwrap();
                    last_section.set_arrival_stop_id(new_section.arrival_stop_id());
                    last_section.set_arrival_at(new_section.arrival_at());
                } else {
                    cloned_sections.push(new_section);
                }

                cloned_visited_stops.extend(new_visited_stops);
            });
            Some(new_route)
        })
    }

    pub fn to_route_result_reverse(&self, data_storage: &DataStorage) -> RouteResult {
        // In reverse mode, sections are stored [Dest->stop1, stop1->stop2, ..., stopN->Origin]
        // We need to reverse this to [Origin->stopN, ..., stop2->stop1, stop1->Dest]
        // AND flip the sections themselves (dep<->arr).

        let sections: Vec<_> = self
            .sections()
            .iter()
            .rev()
            .map(|section| {
                // section is B -> A (reverse step)
                // We want A -> B (physical step)
                let physical_dep_stop = section.arrival_stop_id();
                let physical_arr_stop = section.departure_stop_id();

                // Let's implement manual conversion for reverse sections to avoid hacky swapping.
                // Or better, let's create a physical section.
                // If section is walking: duration is set.
                // If section is journey: use journey to get times.

                if section.is_walking_trip() {
                    // Walking: direction is reversible.
                    let arrival_at_dest = section.arrival_at()
                        + chrono::Duration::minutes(section.duration().unwrap_or(0) as i64);
                    // Wait, section.arrival_at is time at A (start of walk).
                    // So time at B (end of walk) is A + duration.

                    let dep_stop_obj = data_storage.stops().find(physical_dep_stop);
                    let arr_stop_obj = data_storage.stops().find(physical_arr_stop);

                    RouteSectionResult::new(
                        None,
                        physical_dep_stop,
                        dep_stop_obj
                            .map(|s| s.lv95_coordinates())
                            .unwrap_or_default(),
                        dep_stop_obj
                            .map(|s| s.wgs84_coordinates())
                            .unwrap_or_default(),
                        physical_arr_stop,
                        arr_stop_obj
                            .map(|s| s.lv95_coordinates())
                            .unwrap_or_default(),
                        arr_stop_obj
                            .map(|s| s.wgs84_coordinates())
                            .unwrap_or_default(),
                        Some(section.arrival_at()), // Dep at A
                        Some(arrival_at_dest),      // Arr at B
                        section.duration(),
                        Transport::Walk,
                    )
                } else {
                    // Journey
                    let journey = section.journey(data_storage).unwrap();
                    let dep_time = section.arrival_at(); // Time at A (physical dep)

                    // We need arrival time at B.
                    // We can ask the journey.
                    let arr_time = journey
                        .arrival_at_of_with_origin(
                            physical_arr_stop,
                            dep_time.date(),
                            true, // dep_time is departure time.
                            physical_dep_stop,
                        )
                        .or_else(|_| {
                            // Fallback: try next day if close to midnight?
                            journey.arrival_at_of_with_origin(
                                physical_arr_stop,
                                dep_time.date().succ_opt().unwrap_or(dep_time.date()),
                                true,
                                physical_dep_stop,
                            )
                        })
                        .ok();

                    let dep_stop_obj = data_storage.stops().find(physical_dep_stop);
                    let arr_stop_obj = data_storage.stops().find(physical_arr_stop);

                    RouteSectionResult::new(
                        section.journey_id(),
                        physical_dep_stop,
                        dep_stop_obj
                            .map(|s| s.lv95_coordinates())
                            .unwrap_or_default(),
                        dep_stop_obj
                            .map(|s| s.wgs84_coordinates())
                            .unwrap_or_default(),
                        physical_arr_stop,
                        arr_stop_obj
                            .map(|s| s.lv95_coordinates())
                            .unwrap_or_default(),
                        arr_stop_obj
                            .map(|s| s.wgs84_coordinates())
                            .unwrap_or_default(),
                        Some(dep_time),
                        arr_time.or(Some(dep_time)), // Fallback to avoid panic
                        section.duration(),
                        journey
                            .transport_type(data_storage)
                            .map(Transport::from)
                            .unwrap_or(Transport::Train),
                    )
                }
            })
            .collect();

        let departure_at = sections
            .first()
            .and_then(|s| s.departure_at())
            .unwrap_or_else(|| panic!("No departure time for route"));
        let arrival_at = sections
            .last()
            .and_then(|s| s.arrival_at())
            .unwrap_or_else(|| panic!("No arrival time for route"));

        RouteResult::new(departure_at, arrival_at, sections)
    }
}

impl RouteSection {
    pub fn find_previous(
        data_storage: &DataStorage,
        journey: &Journey,
        arrival_stop_id: i32, // The stop we are currently AT (end of physical segment)
        date: NaiveDate,
        is_arrival_date: bool,
    ) -> Option<(RouteSection, FxHashSet<i32>)> {
        // Collect route entries before the FIRST occurrence of arrival_stop_id.
        // Using forward iteration ensures we match the first occurrence, which is
        // important for journeys that loop back through the same stop.
        let before_stop: Vec<_> = journey
            .route()
            .iter()
            .take_while(|entry| entry.stop_id() != arrival_stop_id)
            .collect();

        let mut visited_stops = FxHashSet::default();

        // Iterate backwards through entries before our stop to find the previous exchange point
        for route_entry in before_stop.iter().rev() {
            let stop = route_entry
                .stop(data_storage)
                .unwrap_or_else(|_| panic!("Missing stop on route entry: {route_entry:?}"));
            visited_stops.insert(stop.id());

            if stop.can_be_used_as_exchange_point()
                || journey.is_first_stop(stop.id(), false).unwrap_or(false)
            {
                let departure_at = journey.departure_at_of_with_origin(
                    stop.id(),
                    date,
                    is_arrival_date,
                    arrival_stop_id,
                );

                if let Err(e) = departure_at {
                    log::debug!("Failed to find departure at {}: {:?}", stop.id(), e);
                    continue;
                }
                let departure_at = departure_at.unwrap();

                return Some((
                    RouteSection::new(
                        Some(journey.id()),
                        arrival_stop_id, // Departure in Reverse = Physical Arrival
                        stop.id(),       // Arrival in Reverse = Physical Departure
                        departure_at,    // Time at Frontier (Physical Departure)
                        None,
                    ),
                    visited_stops,
                ));
            }
        }

        None
    }

    pub fn find_next(
        data_storage: &DataStorage,
        journey: &Journey,
        departure_stop_id: i32,
        date: NaiveDate,
        is_departure_date: bool,
    ) -> Option<(RouteSection, FxHashSet<i32>)> {
        let mut route_iter = journey.route().iter();

        for route_entry in route_iter.by_ref() {
            if route_entry.stop_id() == departure_stop_id {
                break;
            }
        }

        let mut visited_stops = FxHashSet::default();

        for route_entry in route_iter.by_ref() {
            let stop = route_entry
                .stop(data_storage)
                .unwrap_or_else(|_| panic!("Missing stop on route entry: {route_entry:?}"));
            visited_stops.insert(stop.id());

            if stop.can_be_used_as_exchange_point()
                || journey.is_last_stop(stop.id(), false).unwrap_or(false)
            {
                let arrival_at = journey.arrival_at_of_with_origin(
                    stop.id(),
                    date,
                    is_departure_date,
                    departure_stop_id,
                ).unwrap_or_else(|_| panic!("No arrival date for stop id: {}, date: {date}, is_departure_date: {is_departure_date}, departure_stop_id: {departure_stop_id}", stop.id()));

                return Some((
                    RouteSection::new(
                        Some(journey.id()),
                        departure_stop_id,
                        stop.id(),
                        arrival_at,
                        None,
                    ),
                    visited_stops,
                ));
            }
        }

        None
    }

    pub fn to_route_section_result(&self, data_storage: &DataStorage) -> RouteSectionResult {
        let departure_stop = data_storage
            .stops()
            .find(self.departure_stop_id())
            .unwrap_or_else(|| panic!("Departure stop {} not found.", self.departure_stop_id()));
        let arrival_stop = data_storage
            .stops()
            .find(self.arrival_stop_id())
            .unwrap_or_else(|| panic!("Arrival stop {} not found.", self.arrival_stop_id()));

        let (departure_at, arrival_at) = if self.journey_id().is_some() {
            let departure_at = self
                .journey(data_storage)
                .unwrap()
                .departure_at_of_with_origin(
                    departure_stop.id(),
                    self.arrival_at().date(),
                    false,
                    arrival_stop.id(),
                )
                .unwrap_or_else(|_| {
                    panic!(
                        "No departure at from {}, at date {}, and origin {}",
                        departure_stop.id(),
                        self.arrival_at().date(),
                        arrival_stop.id()
                    )
                });
            (Some(departure_at), Some(self.arrival_at()))
        } else {
            (None, None)
        };
        let transport = self
            .journey(data_storage)
            .map(|j| j.transport_type(data_storage));
        let transport = if let Some(t) = transport {
            Transport::from(t.unwrap_or_else(|e| panic!("Transport Type not found, {e}")))
        } else {
            Transport::Walk
        };

        RouteSectionResult::new(
            self.journey_id(),
            departure_stop.id(),
            departure_stop.lv95_coordinates(),
            departure_stop.wgs84_coordinates(),
            arrival_stop.id(),
            arrival_stop.lv95_coordinates(),
            arrival_stop.wgs84_coordinates(),
            departure_at,
            arrival_at,
            self.duration(),
            transport,
        )
    }
}
