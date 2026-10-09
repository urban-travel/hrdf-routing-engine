//! The journey planner API: stop search and connections, with the time routing took.

use std::{cmp::Reverse, time::Instant};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use hrdf_parser::Timetable;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::{Route, plan_journey, plan_journey_reverse, service::AppStateRef};

/// What the planner looks up besides the timetable.
pub struct Index {
    /// The words of the names of the stops served by a journey: stations (named "Zürich HB",
    /// where local stops are "Zürich, Central") first, then the most served.
    stop_names: Vec<(Vec<String>, i32)>,
    /// Through services (DURCHBI) as (journey number, next journey number, stop): the vehicle
    /// goes on under another number. ponytail: administrations are ignored, as two operators
    /// rarely link the same journey numbers at the same stop.
    through_services: FxHashSet<(i32, i32, i32)>,
}

impl Index {
    pub fn new(tt: &Timetable) -> Self {
        let mut calls: FxHashMap<i32, u32> = FxHashMap::default();
        let stops = tt.journeys.iter().flat_map(|j| &j.stops);
        stops.for_each(|s| *calls.entry(s.stop_id).or_default() += 1);
        let mut names: Vec<_> = (calls.into_iter())
            .filter_map(|(id, n)| {
                tt.stops
                    .get(&id)
                    .map(|s| (s.name.contains(','), Reverse(n), id))
            })
            .collect();
        names.sort_unstable();
        let through = tt
            .through_services
            .iter()
            .map(|t| (t.from.0, t.to.0, t.stop_id));
        Self {
            through_services: through.collect(),
            stop_names: names
                .into_iter()
                .map(|(.., id)| (words(&tt.stops[&id].name), id))
                .collect(),
        }
    }
}

/// Lowercase words without accents, so that "zurich" finds "Zürich" and "aero" "Genève-Aéroport".
fn words(name: &str) -> Vec<String> {
    let plain = name.chars().flat_map(char::to_lowercase).map(|c| match c {
        'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        c if c.is_alphanumeric() => c,
        _ => ' ',
    });
    plain
        .collect::<String>()
        .split_whitespace()
        .map(String::from)
        .collect()
}

#[derive(Deserialize)]
pub struct StopsRequest {
    q: String,
}

#[derive(Serialize)]
pub struct Place {
    id: i32,
    name: String,
}

/// Stops whose name words start with every query word, those whose name starts with the
/// query first.
pub async fn stops(
    State(state): AppStateRef,
    Query(request): Query<StopsRequest>,
) -> Json<Vec<Place>> {
    let query = words(&request.q);
    let Some(first) = query.first() else {
        return Json(Vec::new());
    };
    let matches = |name: &[String]| query.iter().all(|q| name.iter().any(|w| w.starts_with(q)));
    let found = state
        .index
        .stop_names
        .iter()
        .filter(|(name, _)| matches(name));
    let (mut starting, others): (Vec<_>, Vec<_>) =
        found.partition(|(name, _)| name[0].starts_with(first));
    starting.extend(others);
    Json(
        starting
            .iter()
            .take(10)
            .map(|&&(_, id)| place(&state.router.timetable, id))
            .collect(),
    )
}

fn place(tt: &Timetable, id: i32) -> Place {
    Place {
        id,
        name: tt
            .stops
            .get(&id)
            .map_or_else(|| id.to_string(), |s| s.name.clone()),
    }
}

#[derive(Deserialize)]
pub struct JourneysRequest {
    from: i32,
    to: i32,
    date: NaiveDate,
    time: NaiveTime,
    /// The time is the latest arrival instead of the earliest departure.
    #[serde(default)]
    arrival: bool,
    count: Option<usize>,
}

#[derive(Serialize)]
pub struct JourneysResponse {
    connections: Vec<Connection>,
    /// Building the day's network, done once per date.
    prepare_ms: f64,
    routing_ms: f64,
}

#[derive(Serialize)]
pub struct Connection {
    departure: NaiveDateTime,
    arrival: NaiveDateTime,
    /// Vehicle changes: staying on board of a through service is not one.
    changes: usize,
    routing_ms: f64,
    sections: Vec<Section>,
}

/// A ride, or a walk when `ride` is absent.
#[derive(Serialize)]
pub struct Section {
    from: Place,
    to: Place,
    departure: NaiveDateTime,
    arrival: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    ride: Option<Ride>,
}

#[derive(Serialize)]
pub struct Ride {
    transport: String,
    /// The English name of the transport, e.g. "Bus" for "B".
    category: String,
    line: String,
    number: i32,
    operator: String,
    direction: String,
    /// The previous ride's vehicle goes on as this one: no change.
    continues: bool,
    stops: Vec<Call>,
}

/// Times are "HH:MM".
#[derive(Serialize)]
pub struct Call {
    id: i32,
    name: String,
    arrival: Option<String>,
    departure: Option<String>,
    /// Track, bus bay...
    platform: Option<String>,
}

/// The next `count` connections leaving after (or arriving before) the requested time.
pub async fn journeys(
    State(state): AppStateRef,
    Query(request): Query<JourneysRequest>,
) -> Result<Json<JourneysResponse>, StatusCode> {
    let (router, tt) = (&state.router, &state.router.timetable);
    if request.date < tt.start || request.date > tt.end {
        return Err(StatusCode::BAD_REQUEST);
    }
    let started = Instant::now();
    router.prepare(request.date);
    let prepare_ms = ms(started);

    let mut at = NaiveDateTime::new(request.date, request.time);
    let mut connections = Vec::new();
    for _ in 0..request.count.unwrap_or(4).min(10) {
        let started = Instant::now();
        let route = if request.arrival {
            plan_journey_reverse(router, request.from, request.to, at, 10, false)
        } else {
            plan_journey(router, request.from, request.to, at, 10, false)
        };
        let Some(route) = route else { break };
        at = if request.arrival {
            route.arrival_at - Duration::minutes(1)
        } else {
            route.departure_at + Duration::minutes(1)
        };
        connections.push(connection(tt, &state.index, &route, ms(started)));
    }
    if request.arrival {
        connections.reverse();
    }
    let routing_ms = connections.iter().map(|c| c.routing_ms).sum();
    Ok(Json(JourneysResponse {
        connections,
        prepare_ms,
        routing_ms,
    }))
}

fn ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn connection(tt: &Timetable, index: &Index, route: &Route, routing_ms: f64) -> Connection {
    let hhmm = |t: Option<i16>| t.map(|m| format!("{:02}:{:02}", m / 60 % 24, m % 60));
    let mut sections: Vec<Section> = Vec::new();
    for s in &route.sections {
        let ride = s.journey.map(|(j, first, last)| {
            let journey = &tt.journeys[j];
            // A through service links the journey ridden just before (with no walk between).
            let previous = sections
                .last()
                .and_then(|p| p.ride.as_ref())
                .map(|r| r.number);
            let through = |p: i32| (p, journey.number, s.departure_stop_id);
            let calls = journey.stops[first as usize..=last as usize].iter();
            let calls = calls.filter(|c| c.can_board || c.can_alight).map(|c| Call {
                id: c.stop_id,
                name: place(tt, c.stop_id).name,
                arrival: hhmm(c.arrival),
                departure: hhmm(c.departure),
                platform: c
                    .platform
                    .and_then(|p| tt.platforms.get(&(c.stop_id, p.get())).cloned()),
            });
            Ride {
                continues: previous.is_some_and(|p| index.through_services.contains(&through(p))),
                transport: journey.transport.clone(),
                category: tt
                    .transport_names
                    .get(&journey.transport)
                    .unwrap_or(&journey.transport)
                    .clone(),
                line: journey
                    .line
                    .map_or(journey.number.to_string(), |l| tt.lines[l as usize].clone()),
                number: journey.number,
                operator: tt
                    .operators
                    .get(&journey.admin)
                    .unwrap_or(&journey.admin)
                    .clone(),
                direction: match journey.towards {
                    Some(d) => tt.directions[d as usize].clone(),
                    None => place(tt, journey.stops.last().map_or(0, |s| s.stop_id)).name,
                },
                stops: calls.collect(),
            }
        });
        sections.push(Section {
            from: place(tt, s.departure_stop_id),
            to: place(tt, s.arrival_stop_id),
            departure: s.departure_at,
            arrival: s.arrival_at,
            ride,
        });
    }
    let rides = sections.iter().filter_map(|s| s.ride.as_ref());
    let changes = rides.filter(|r| !r.continues).count().saturating_sub(1);
    Connection {
        departure: route.departure_at,
        arrival: route.arrival_at,
        changes,
        routing_ms,
        sections,
    }
}
