//! Compares `plan_journey` with OJP, the Swiss national journey planner.
//!
//! `cargo run --release --example ojp -- compare <hrdf.zip> [dir of OJP trip responses]`
//!
//! For every trip OJP returned, our engine is asked for the earliest arrival when leaving at the
//! same stop and minute, and compared with the best arrival among the OJP trips leaving then or
//! later. On the same timetable, arriving later means we miss a connection, and arriving earlier
//! usually means we break a rule OJP knows about.
//!
//! `cargo run --release --example ojp -- fetch <hrdf.zip> <dir> [count]` records `count` OJP
//! responses (default 300) between random Swiss stops served on the next Tuesday and Saturday,
//! which OJP plans without realtime data. It needs `OJP_TOKEN` (in `.env`), from
//! <https://api-manager.opentransportdata.swiss>. Compare against the timetable delivery that was
//! current when fetching: both change with every delivery.

use std::{error::Error, fs, time::Instant};

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike, Weekday};
use chrono_tz::Europe::Zurich;
use hrdf_parser::Timetable;
use hrdf_routing_engine::{Router, plan_journey};
use ojp_rs::{OJP, RequestBuilder, RequestType, SimplifiedTrip};

/// Local time, to the minute: OJP adds seconds to walks.
fn local(utc: NaiveDateTime) -> NaiveDateTime {
    utc.and_utc()
        .with_timezone(&Zurich)
        .naive_local()
        .with_second(0)
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: ojp compare <hrdf.zip> [dir] | ojp fetch <hrdf.zip> <dir> [count]";
    let (Some(command), Some(zip)) = (args.first(), args.get(1)) else {
        return Err(usage.into());
    };
    let started = Instant::now();
    let router = Router::new(Timetable::path(zip).cache_dir("cache").load().await?);
    println!("Timetable loaded in {:?}", started.elapsed());
    let dir = args.get(2).map_or("test_xml", |d| d.as_str());
    match command.as_str() {
        "compare" => compare(&router, dir),
        "fetch" => fetch(&router, dir, args.get(3).map_or(Ok(300), |c| c.parse())?).await,
        _ => Err(usage.into()),
    }
}

fn compare(router: &Router, dir: &str) -> Result<(), Box<dyn Error>> {
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    paths.sort();
    let (mut same, mut later, mut earlier, mut missing) = (0, 0, 0, 0);
    let mut times = Vec::new();
    for path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
    {
        let ojp = OJP::try_from(fs::read_to_string(path)?.as_str())?;
        let trips = ojp
            .trips()
            .unwrap_or_default()
            .into_iter()
            .map(|r| SimplifiedTrip::try_from(r.trip()));
        let trips = trips.collect::<Result<Vec<_>, _>>()?;
        for trip in &trips {
            let (from, to) = (trip.departure_id()?, trip.arrival_id()?);
            let departure = local(trip.departure_time()?);
            let same_ends = |t: &&SimplifiedTrip| {
                t.departure_id().ok() == Some(from) && t.arrival_id().ok() == Some(to)
            };
            let leaving_later =
                |t: &&SimplifiedTrip| t.departure_time().is_ok_and(|d| local(d) >= departure);
            let best = trips
                .iter()
                .filter(same_ends)
                .filter(leaving_later)
                .filter_map(|t| t.arrival_time().ok());
            let arrival = local(best.min().unwrap());

            // The best of three runs: timings on a busy machine vary a lot.
            let mut route = None;
            let time = (0..3).map(|_| {
                let started = Instant::now();
                route = plan_journey(router, from, to, departure, 10, false);
                started.elapsed()
            });
            times.push(time.min().unwrap());

            let name = path.file_name().unwrap().to_string_lossy();
            match route {
                None => {
                    missing += 1;
                    println!(
                        "NOT FOUND {name}: {from} -> {to} at {departure}, OJP arrives {arrival}"
                    );
                }
                Some(route) if route.arrival_at == arrival => same += 1,
                Some(route) => {
                    let delta = (route.arrival_at - arrival).num_minutes();
                    if delta > 0 {
                        later += 1
                    } else {
                        earlier += 1
                    }
                    println!(
                        "{delta:+} min {name}: {from} -> {to} at {departure}, OJP arrives {arrival}"
                    );
                    println!("OJP:\n{trip}");
                    println!("Ours:");
                    route.print(&router.timetable);
                    println!();
                }
            }
        }
    }

    let total = same + later + earlier + missing;
    println!(
        "\n{total} trips: {same} same arrival, {later} we arrive later, {earlier} we arrive earlier, {missing} not found"
    );
    times.sort();
    if !times.is_empty() {
        let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
        println!(
            "Query time p50 {:?}, p95 {:?}, max {:?}",
            at(0.5),
            at(0.95),
            at(1.0)
        );
    }
    Ok(())
}

async fn fetch(router: &Router, dir: &str, count: usize) -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let key = std::env::var("OJP_TOKEN").map_err(|_| "OJP_TOKEN is not set (see .env)")?;
    fs::create_dir_all(dir)?;
    let today = Local::now().date_naive();
    let next = |weekday| {
        (2..9)
            .map(|d| today + Duration::days(d))
            .find(|d| d.weekday() == weekday)
    };
    let dates: Vec<NaiveDate> = [Weekday::Tue, Weekday::Sat]
        .into_iter()
        .filter_map(next)
        .collect();
    // Swiss stops where someone can board, then alight, on each date.
    let tt = &router.timetable;
    let served = |date: NaiveDate| {
        let mut stops: Vec<(i32, bool, bool)> = Vec::new();
        for journey in &tt.journeys {
            if journey
                .sections
                .iter()
                .any(|s| tt.runs_on(s.bit_field, date))
            {
                let calls = journey.stops.iter().filter(|c| c.stop_id / 100_000 == 85);
                stops.extend(calls.map(|c| (c.stop_id, c.can_board, c.can_alight)));
            }
        }
        stops.sort_unstable();
        stops.dedup();
        let origins: Vec<i32> = stops.iter().filter(|s| s.1).map(|s| s.0).collect();
        let destinations: Vec<i32> = stops.iter().filter(|s| s.2).map(|s| s.0).collect();
        (origins, destinations)
    };
    let served: Vec<_> = dates.iter().map(|&d| served(d)).collect();

    // xorshift64: a fixed sequence of queries.
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut random = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    for i in 0..count {
        let (date, (origins, destinations)) = (dates[i % dates.len()], &served[i % dates.len()]);
        let (from, to) = (
            origins[random(origins.len())],
            destinations[random(destinations.len())],
        );
        let at = date
            .and_hms_opt(5 + random(17) as u32, random(60) as u32, 0)
            .unwrap();
        // ojp-rs reads the time in this machine's time zone.
        let at_here = at
            .and_local_timezone(Zurich)
            .single()
            .ok_or("ambiguous time")?
            .with_timezone(&Local);
        let request = RequestBuilder::try_new(at_here.naive_local())?
            .set_request_type(RequestType::Trip)
            .set_from(from)
            .set_to(to)
            .set_number_results(6)
            .set_requestor_ref("hrdf-routing-engine")
            .set_token(ojp_rs::token(&key)?);
        let xml = request
            .build_request()?
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        fs::write(format!("{dir}/{from}_{to}_trip.xml"), xml)?;
        println!("{}/{count}: {from} -> {to} at {at}", i + 1);
        // Stay below the API rate limit.
        std::thread::sleep(std::time::Duration::from_millis(1200));
    }
    Ok(())
}
