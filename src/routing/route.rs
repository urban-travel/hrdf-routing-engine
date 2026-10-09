use chrono::{Duration, NaiveDate, NaiveDateTime};
use hrdf_parser::{Coordinates, StopTime, Timetable};

use super::{Router, raptor::Leg};

#[derive(Clone, Debug)]
pub struct Route {
    pub departure_at: NaiveDateTime,
    pub arrival_at: NaiveDateTime,
    pub sections: Vec<RouteSection>,
}

#[derive(Clone, Debug)]
pub struct RouteSection {
    /// (index into `Timetable::journeys`, first and last stop index ridden); None for a walk.
    pub journey: Option<(usize, u16, u16)>,
    pub departure_stop_id: i32,
    pub arrival_stop_id: i32,
    pub departure_at: NaiveDateTime,
    pub arrival_at: NaiveDateTime,
    pub departure_stop_lv95: Option<Coordinates>,
    pub arrival_stop_lv95: Option<Coordinates>,
}

impl Route {
    /// `start` is used for the times of a route made only of a walk.
    pub(crate) fn new(
        router: &Router,
        date: NaiveDate,
        legs: &[Leg],
        start: NaiveDateTime,
    ) -> Self {
        let midnight = date.and_hms_opt(0, 0, 0).unwrap();
        let tt = &router.timetable;
        let ride_times = |leg: &Leg| match *leg {
            Leg::Ride { trip, first, last } => {
                let stops = &tt.journeys[trip.journey as usize].stops;
                let at = |minutes: Option<i16>| {
                    midnight
                        + Duration::minutes(
                            i64::from(minutes.unwrap()) + 1440 * i64::from(trip.day),
                        )
                };
                let (a, b): (&StopTime, &StopTime) =
                    (&stops[first as usize], &stops[last as usize]);
                Some((at(a.departure.or(a.arrival)), at(b.arrival.or(b.departure))))
            }
            Leg::Walk { .. } => None,
        };

        let mut sections: Vec<RouteSection> = Vec::with_capacity(legs.len());
        for (i, leg) in legs.iter().enumerate() {
            let (journey, from, to, departure_at, arrival_at) = match *leg {
                Leg::Ride { trip, first, last } => {
                    let stops = &tt.journeys[trip.journey as usize].stops;
                    let (departure_at, arrival_at) = ride_times(leg).unwrap();
                    let journey = Some((trip.journey as usize, first, last));
                    (
                        journey,
                        stops[first as usize].stop_id,
                        stops[last as usize].stop_id,
                        departure_at,
                        arrival_at,
                    )
                }
                Leg::Walk { from, to, minutes } => {
                    let minutes = Duration::minutes(minutes.into());
                    // Walk right after the previous ride, or just in time for the next one.
                    let departure_at = match (sections.last(), legs.get(i + 1).and_then(ride_times))
                    {
                        (Some(previous), _) => previous.arrival_at,
                        (None, Some((next_departure, _))) => next_departure - minutes,
                        (None, None) => start,
                    };
                    let (from, to) = (router.stop_ids[from as usize], router.stop_ids[to as usize]);
                    (None, from, to, departure_at, departure_at + minutes)
                }
            };
            sections.push(RouteSection {
                journey,
                departure_stop_id: from,
                arrival_stop_id: to,
                departure_at,
                arrival_at,
                departure_stop_lv95: tt.stops.get(&from).and_then(|s| s.lv95),
                arrival_stop_lv95: tt.stops.get(&to).and_then(|s| s.lv95),
            });
        }
        Route {
            departure_at: sections.first().map_or(start, |s| s.departure_at),
            arrival_at: sections.last().map_or(start, |s| s.arrival_at),
            sections,
        }
    }

    pub fn print(&self, tt: &Timetable) {
        let name = |id: i32| tt.stops.get(&id).map_or("?", |s| s.name.as_str());
        let hhmm = |t: Option<i16>| {
            t.map_or("     ".into(), |m| {
                format!("{:02}:{:02}", m / 60 % 24, m % 60)
            })
        };
        for section in &self.sections {
            let Some((journey, first, last)) = section.journey else {
                let minutes = (section.arrival_at - section.departure_at).num_minutes();
                println!(
                    "Approx. {minutes}-minute walk to {}",
                    name(section.arrival_stop_id)
                );
                continue;
            };
            let journey = &tt.journeys[journey];
            let line = journey
                .line
                .map_or(journey.number.to_string(), |l| tt.lines[l as usize].clone());
            let operator = tt.operators.get(&journey.admin).unwrap_or(&journey.admin);
            let towards = journey.towards.map_or_else(String::new, |d| {
                format!(" towards {}", tt.directions[d as usize])
            });
            println!(
                "{} {line}{towards} (journey {}, {operator})",
                journey.transport, journey.number
            );
            println!(
                "  Departure at: {}",
                section.departure_at.format("%Y-%m-%d %H:%M")
            );
            // Points passed without stopping (tunnels, lines) cannot be boarded nor left.
            let calls = journey.stops[first as usize..=last as usize].iter();
            for call in calls.filter(|c| c.can_board || c.can_alight) {
                let (arrival, departure) = (hhmm(call.arrival), hhmm(call.departure));
                println!(
                    "    {:0>7} {: <36} {arrival} - {departure}",
                    call.stop_id,
                    name(call.stop_id)
                );
            }
            println!(
                "  Arrival at: {}",
                section.arrival_at.format("%Y-%m-%d %H:%M")
            );
        }
    }
}
