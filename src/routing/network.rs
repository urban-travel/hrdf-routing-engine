//! The trips of one day grouped into RAPTOR patterns: trips calling at the same stops that never
//! overtake each other, so the first trip catchable at a stop is also the first everywhere after.

use chrono::{Duration, NaiveDate};
use hrdf_parser::Timetable;
use rustc_hash::FxHashMap;

/// ponytail: trips of the day before ending before 18:00 and of the day after starting after
/// 06:00 are left out, which keeps searches a third faster. A journey needing them (more than six
/// hours across midnight) is not found; widen the window if that matters.
const WINDOW: i32 = 6 * 60;

/// A call of a trip at a stop. Times are minutes from midnight of the network's date.
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub arrival: i32,
    pub departure: i32,
    pub can_board: bool,
    pub can_alight: bool,
    /// Index of the stop in the journey.
    pub pos: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct Trip {
    /// Index into `Timetable::journeys`.
    pub journey: u32,
    /// Service day relative to the network's date (-1, 0 or 1).
    pub day: i8,
}

/// The events of trip `t` (counted from 0 in the pattern) are `events[first_event + t * len..][..len]`.
#[derive(Debug)]
pub struct Pattern {
    pub first_stop: usize,
    pub first_trip: usize,
    pub first_event: usize,
    pub len: usize,
    pub trips: usize,
}

/// A backward network is a forward one with time running backwards: patterns and trips are
/// reversed, times negated, boarding and alighting swapped. Searching it for the earliest
/// (negated) arrival finds the latest departure.
pub struct Network {
    pub date: NaiveDate,
    pub backward: bool,
    pub patterns: Vec<Pattern>,
    /// Stop indexes of each pattern, consecutive.
    pub stops: Vec<u32>,
    pub events: Vec<Event>,
    pub trips: Vec<Trip>,
    /// (pattern, position) of every call at a stop.
    pub stop_patterns: Vec<Vec<(u32, u32)>>,
}

impl Network {
    pub fn event(&self, pattern: &Pattern, trip: usize, pos: usize) -> &Event {
        &self.events[pattern.first_event + trip * pattern.len + pos]
    }

    pub fn stop(&self, pattern: &Pattern, pos: usize) -> u32 {
        self.stops[pattern.first_stop + pos]
    }

    /// The trips running on `date`, plus those of the days before and after (shifted by a day)
    /// that run within `WINDOW` of it, so that searches can cross midnight in both directions.
    pub fn build(tt: &Timetable, stop_index: &FxHashMap<i32, u32>, date: NaiveDate) -> Self {
        struct Candidate {
            trip: Trip,
            events: Vec<Event>,
        }
        let mut groups: FxHashMap<Vec<u32>, Vec<Candidate>> = FxHashMap::default();
        let mut push = |stops: &mut Vec<u32>, events: &mut Vec<Event>, trip: Trip| {
            let in_window =
                |e: &[Event]| e[e.len() - 1].arrival >= -WINDOW && e[0].departure <= 1440 + WINDOW;
            if stops.len() >= 2 && in_window(events) {
                let events = std::mem::take(events);
                groups
                    .entry(std::mem::take(stops))
                    .or_default()
                    .push(Candidate { trip, events });
            }
            stops.clear();
            events.clear();
        };

        // Whether each bit field runs on the day before, the day and the day after, by id.
        let max_id = tt.bit_fields.keys().copied().max().unwrap_or(0).max(0) as usize;
        let runs: Vec<Vec<bool>> = (-1..=1)
            .map(|day| {
                let service_date = date + Duration::days(day);
                let mut runs = vec![false; max_id + 1];
                runs[0] = tt.runs_on(0, service_date);
                tt.bit_fields
                    .keys()
                    .for_each(|&id| runs[id as usize] = tt.runs_on(id, service_date));
                runs
            })
            .collect();

        for (journey_index, journey) in tt.journeys.iter().enumerate() {
            for day in -1..=1i8 {
                let runs = &runs[(day + 1) as usize];
                let running: Vec<_> = (journey.sections.iter())
                    .filter(|s| runs.get(s.bit_field as usize).copied().unwrap_or(false))
                    .collect();
                if running.is_empty() {
                    continue;
                }
                let trip = Trip {
                    journey: journey_index as u32,
                    day,
                };
                let shift = i32::from(day) * 1440;
                let (mut stops, mut events) = (Vec::new(), Vec::new());
                for (i, call) in journey.stops.iter().enumerate() {
                    // The vehicle only runs where one of today's sections covers the stop.
                    if !running
                        .iter()
                        .any(|s| (s.first as usize..=s.last as usize).contains(&i))
                    {
                        push(&mut stops, &mut events, trip);
                        continue;
                    }
                    let (Some(&stop), Some(arrival), Some(departure)) = (
                        stop_index.get(&call.stop_id),
                        call.arrival.or(call.departure),
                        call.departure.or(call.arrival),
                    ) else {
                        continue;
                    };
                    if call.can_board || call.can_alight {
                        stops.push(stop);
                        events.push(Event {
                            arrival: i32::from(arrival) + shift,
                            departure: i32::from(departure) + shift,
                            can_board: call.can_board,
                            can_alight: call.can_alight,
                            pos: i as u16,
                        });
                    }
                }
                push(&mut stops, &mut events, trip);
            }
        }

        let mut net = Network {
            date,
            backward: false,
            patterns: Vec::new(),
            stops: Vec::new(),
            events: Vec::new(),
            trips: Vec::new(),
            stop_patterns: vec![Vec::new(); stop_index.len()],
        };
        for (stops, mut candidates) in groups {
            candidates.sort_by_key(|c| (c.events[0].departure, c.events[stops.len() - 1].arrival));
            // Greedily split into groups of trips that do not overtake each other.
            let mut parts: Vec<Vec<Candidate>> = Vec::new();
            for candidate in candidates {
                let fits = |part: &&mut Vec<Candidate>| {
                    let last = &part.last().unwrap().events;
                    (last.iter().zip(&candidate.events))
                        .all(|(a, b)| a.arrival <= b.arrival && a.departure <= b.departure)
                };
                match parts.iter_mut().find(fits) {
                    Some(part) => part.push(candidate),
                    None => parts.push(vec![candidate]),
                }
            }
            for part in parts {
                net.push_pattern(&stops, part.iter().map(|c| (c.trip, &c.events[..])));
            }
        }
        net
    }

    /// The same network with time running backwards.
    pub fn reversed(&self) -> Self {
        let mut net = Network {
            date: self.date,
            backward: !self.backward,
            patterns: Vec::with_capacity(self.patterns.len()),
            stops: Vec::with_capacity(self.stops.len()),
            events: Vec::with_capacity(self.events.len()),
            trips: Vec::with_capacity(self.trips.len()),
            stop_patterns: vec![Vec::new(); self.stop_patterns.len()],
        };
        for p in &self.patterns {
            let stops: Vec<_> = self.stops[p.first_stop..][..p.len]
                .iter()
                .rev()
                .copied()
                .collect();
            let trips: Vec<(Trip, Vec<Event>)> = (0..p.trips)
                .rev()
                .map(|t| {
                    let events = self.events[p.first_event + t * p.len..][..p.len]
                        .iter()
                        .rev();
                    let events = events.map(|e| Event {
                        arrival: -e.departure,
                        departure: -e.arrival,
                        can_board: e.can_alight,
                        can_alight: e.can_board,
                        pos: e.pos,
                    });
                    (self.trips[p.first_trip + t], events.collect())
                })
                .collect();
            net.push_pattern(
                &stops,
                trips.iter().map(|(trip, events)| (*trip, &events[..])),
            );
        }
        net
    }

    fn push_pattern<'a>(
        &mut self,
        stops: &[u32],
        trips: impl Iterator<Item = (Trip, &'a [Event])>,
    ) {
        let index = self.patterns.len() as u32;
        let mut pattern = Pattern {
            first_stop: self.stops.len(),
            first_trip: self.trips.len(),
            first_event: self.events.len(),
            len: stops.len(),
            trips: 0,
        };
        for (pos, &stop) in stops.iter().enumerate() {
            self.stop_patterns[stop as usize].push((index, pos as u32));
        }
        self.stops.extend(stops);
        for (trip, events) in trips {
            self.trips.push(trip);
            self.events.extend(events);
            pattern.trips += 1;
        }
        self.patterns.push(pattern);
    }
}
