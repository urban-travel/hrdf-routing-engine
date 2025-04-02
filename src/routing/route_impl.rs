use chrono::NaiveDate;
use hrdf_parser::{DataStorage, Journey, Model};
use rustc_hash::FxHashSet;

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
            .unwrap_or_else(|| panic!("Journey {:?} not found.", journey_id));

        if journey.is_last_stop(self.arrival_stop_id(), false) {
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
            if self.has_visited_any_stops(&new_visited_stops)
                && new_section.arrival_stop_id() != journey.first_stop_id()
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
}

impl RouteSection {
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
            let stop = route_entry.stop(data_storage);
            visited_stops.insert(stop.id());

            if stop.can_be_used_as_exchange_point() || journey.is_last_stop(stop.id(), false) {
                let arrival_at = journey.arrival_at_of_with_origin(
                    stop.id(),
                    date,
                    is_departure_date,
                    departure_stop_id,
                );

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
            .unwrap_or_else(|| panic!("Stop {:?} not found.", self.departure_stop_id()));
        let arrival_stop = data_storage
            .stops()
            .find(self.arrival_stop_id())
            .unwrap_or_else(|| panic!("Stop {:?} not found.", self.arrival_stop_id()));

        let (departure_at, arrival_at) = if self.journey_id().is_some() {
            let departure_at = self
                .journey(data_storage)
                .unwrap()
                .departure_at_of_with_origin(
                    departure_stop.id(),
                    self.arrival_at().date(),
                    false,
                    arrival_stop.id(),
                );
            (Some(departure_at), Some(self.arrival_at()))
        } else {
            (None, None)
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
        )
    }
}
