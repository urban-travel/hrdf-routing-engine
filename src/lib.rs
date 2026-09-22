mod app;
mod cli;
mod debug;
mod error;
mod isochrone;
mod journey;
mod routing;
mod service;
mod utils;

#[cfg(feature = "hectare")]
pub use app::run_surface_per_ha;
pub use app::{
    run_average, run_average_reverse, run_comparison, run_optimal, run_optimal_reverse, run_simple,
    run_simple_reverse, run_worst,
};
pub use cli::{Cli, Mode};
pub use debug::run_debug;
pub use error::RResult;
pub use isochrone::externals::{ExcludedPolygons, LAKES_GEOJSON_URLS};
pub use isochrone::{
    IsochroneArgs, IsochroneDisplayMode, ReverseIsochroneArgs, compute_isochrones,
    compute_isochrones_reverse, compute_optimal_isochrones_reverse,
};
#[cfg(feature = "hectare")]
pub use isochrone::{IsochroneHectareArgs, externals::HectareData};
pub use journey::{JourneyArgs, ReverseJourneyArgs};
pub use routing::{Route, plan_journey, plan_journey_reverse, plan_shortest_journey};
pub use service::run_service;

#[cfg(test)]
mod tests {
    use std::{env, error::Error, fs::read_to_string, time::Instant};

    use crate::{
        ExcludedPolygons, HectareData, LAKES_GEOJSON_URLS,
        isochrone::unique_coordinates_from_routes,
        routing::{
            compute_routes_from_origin, find_origin_stops_within_time_limit,
            find_reachable_stops_within_time_limit, plan_shortest_journey_with_reverse,
        },
        utils::create_date_time,
    };
    use chrono::{Duration, NaiveDateTime, TimeDelta, Timelike};
    use chrono_tz::Europe::Zurich;
    use hrdf_parser::Hrdf;
    use ojp_rs::{OJP, SimplifiedLeg, SimplifiedTrip};

    use test_log::test;

    use crate::{Route, plan_journey, plan_journey_reverse, plan_shortest_journey};
    use futures::future::join_all;

    use pretty_assertions::assert_eq;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use serde::{Deserialize, Serialize};

    fn get_json_values<F>(
        lhs: &F,
        rhs: &str,
    ) -> Result<(serde_json::Value, serde_json::Value), Box<dyn Error>>
    where
        for<'a> F: Serialize + Deserialize<'a>,
    {
        let serialized = serde_json::to_string(&lhs)?;
        let reference = serde_json::to_string(&serde_json::from_str::<F>(rhs)?)?;
        Ok((
            serialized.parse::<serde_json::Value>()?,
            reference.parse::<serde_json::Value>()?,
        ))
    }

    fn local_to_utc_naive(local: NaiveDateTime) -> NaiveDateTime {
        local
            .and_local_timezone(Zurich)
            .single()
            .expect("ambiguous or non-existent local time")
            .naive_utc()
    }

    fn utc_naive_to_local(utc: NaiveDateTime) -> NaiveDateTime {
        utc.and_utc().with_timezone(&Zurich).naive_local()
    }

    struct STrip(SimplifiedTrip);

    impl STrip {
        fn from(value: &Route, hrdf: &Hrdf) -> Self {
            let mut prev_arr_time = value.departure_at();
            let legs = value
                .sections()
                .iter()
                .map(|s| {
                    let departure_id = s.departure_stop_id();
                    let departure_stop = s.departure_stop_name(hrdf.data_storage());
                    let arrival_id = s.arrival_stop_id();
                    let arrival_stop = s.arrival_stop_name(hrdf.data_storage());
                    let departure_time = s.departure_at().unwrap_or(prev_arr_time);
                    let arrival_time = s.arrival_at().unwrap_or(
                        prev_arr_time + TimeDelta::minutes(s.duration().unwrap_or(0) as i64),
                    );
                    prev_arr_time = arrival_time;
                    SimplifiedLeg::new(
                        departure_id,
                        departure_stop,
                        arrival_id,
                        arrival_stop,
                        local_to_utc_naive(departure_time),
                        local_to_utc_naive(arrival_time),
                        format!("{:?}", s.transport()),
                    )
                })
                .collect::<Vec<_>>();
            STrip(SimplifiedTrip::try_new(legs).expect("failed to build SimplifiedTrip"))
        }
    }

    static IDS: [(i32, i32); 33] = [
        (8577820, 8501120),
        (8572662, 8576724),
        (8593320, 8579237),
        (8592862, 8500236),
        (8592458, 8595922),
        (8591921, 8589143),
        (8591915, 8583275),
        (8591611, 8595689),
        (8590925, 8592776),
        (8589645, 8583274),
        (8589632, 8591245),
        (8589164, 8592567),
        (8591610, 8575154),
        (8591921, 8581062),
        (8592837, 8588351),
        (8591363, 8504100),
        (8580798, 8588731),
        (8592587, 8593462),
        (8596094, 8589007),
        (8583005, 8591046),
        (8589151, 8592547),
        (8588949, 8580456),
        (8573693, 8504354),
        (8509076, 8587619),
        (8501120, 8579006),
        (8591418, 8592834),
        (8570732, 8573673),
        (8578997, 8576815),
        (8585206, 8506302),
        (8589587, 8592133),
        (8592889, 8589566),
        (8572453, 8591998),
        (8500236, 8511236),
    ];

    pub async fn test_paths_validity_reverse(
        hrdf: &Hrdf,
        ids: &[(i32, i32)],
    ) -> Result<Vec<(Option<SimplifiedTrip>, Option<SimplifiedTrip>)>, Box<dyn Error>> {
        let ref_trips = ids
            .iter()
            .map(|(from_id, to_id)| {
                let fname = format!("test_xml/{from_id}_{to_id}_trip.xml");
                let xml = std::fs::read_to_string(fname).unwrap();
                let ojp = OJP::try_from(xml.as_str()).unwrap();

                let ref_trip = ojp.fastest_trip().unwrap();

                SimplifiedTrip::try_from(ref_trip).unwrap()
            })
            .collect::<Vec<_>>();

        let hrdf_trips = ref_trips
            .iter()
            .map(|st| async move {
                let from_id = st.departure_id().expect("failed to get departure_id");
                let to_id = st.arrival_id().expect("failed to get arrival_id");
                let date_time =
                    utc_naive_to_local(st.departure_time().expect("failed to get departure_time"))
                        .with_second(0)
                        .unwrap();
                log::info!("Testing trip: {from_id} - {to_id} at {date_time}");
                plan_shortest_journey_with_reverse(hrdf, from_id, to_id, date_time, 10, false)
                    .as_ref()
                    .map(|r| STrip::from(r, hrdf).0)
            })
            .collect::<Vec<_>>();
        let hrdf_trips: Vec<_> = join_all(hrdf_trips).await;
        // We are only interested in the "failures" of the hrdf routing engine
        let failed_comparison = ref_trips
            .into_iter()
            .zip(hrdf_trips)
            .filter_map(|(rt, ht)| {
                if let Some(ht) = ht
                    && !rt.approx_equal(&ht, 0.1)
                {
                    Some((Some(rt), Some(ht)))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Ok(failed_comparison)
    }

    pub async fn test_paths_validity_consistency(
        hrdf: &Hrdf,
        ids: &[(i32, i32)],
    ) -> Result<Vec<(Option<SimplifiedTrip>, Option<SimplifiedTrip>)>, Box<dyn Error>> {
        let ref_trips = ids
            .iter()
            .map(|(from_id, to_id)| {
                let fname = format!("test_xml/{from_id}_{to_id}_trip.xml");
                let xml = std::fs::read_to_string(fname).unwrap();
                let ojp = OJP::try_from(xml.as_str()).unwrap();

                let ref_trip = ojp.fastest_trip().unwrap();

                SimplifiedTrip::try_from(ref_trip).unwrap()
            })
            .collect::<Vec<_>>();

        let hrdf_trips = ref_trips
            .iter()
            .map(|st| async move {
                let from_id = st.departure_id().expect("failed to get departure_id");
                let to_id = st.arrival_id().expect("failed to get arrival_id");
                let date_time =
                    utc_naive_to_local(st.departure_time().expect("failed to get departure_time"))
                        .with_second(0)
                        .unwrap();
                log::info!("Testing trip forward: {from_id} - {to_id} at {date_time}");
                plan_shortest_journey(hrdf, from_id, to_id, date_time, 11, false)
                    .as_ref()
                    .map(|r| STrip::from(r, hrdf).0)
            })
            .collect::<Vec<_>>();
        let hrdf_trips: Vec<_> = join_all(hrdf_trips).await;

        let hrdf_trips_reverse = ref_trips
            .iter()
            .map(|st| async move {
                let from_id = st.departure_id().expect("failed to get departure_id");
                let to_id = st.arrival_id().expect("failed to get arrival_id");
                let date_time =
                    utc_naive_to_local(st.departure_time().expect("failed to get departure_time"))
                        .with_second(0)
                        .unwrap();
                log::info!("Testing trip reverse: {from_id} - {to_id} at {date_time}");
                plan_shortest_journey_with_reverse(hrdf, from_id, to_id, date_time, 11, false)
                    .as_ref()
                    .map(|r| STrip::from(r, hrdf).0)
            })
            .collect::<Vec<_>>();
        let hrdf_trips_reverse: Vec<_> = join_all(hrdf_trips_reverse).await;

        // We are only interested in the "failures" of the hrdf routing engine
        let failed_comparison = hrdf_trips
            .into_iter()
            .zip(hrdf_trips_reverse)
            .filter_map(|(rt, ht)| match (rt, ht) {
                (Some(rt), Some(ht)) => {
                    if !rt.approx_equal(&ht, 0.1) {
                        Some((Some(rt), Some(ht)))
                    } else {
                        None
                    }
                }
                (Some(rt), None) => Some((Some(rt), None)),
                (None, Some(rt)) => Some((None, Some(rt))),
                _ => None,
            })
            .collect::<Vec<_>>();

        Ok(failed_comparison)
    }

    pub async fn test_paths_validity(
        hrdf: &Hrdf,
        ids: &[(i32, i32)],
    ) -> Result<Vec<(Option<SimplifiedTrip>, Option<SimplifiedTrip>)>, Box<dyn Error>> {
        let ref_trips = ids
            .iter()
            .map(|(from_id, to_id)| {
                let fname = format!("test_xml/{from_id}_{to_id}_trip.xml");
                let xml = std::fs::read_to_string(fname).unwrap();
                let ojp = OJP::try_from(xml.as_str()).unwrap();

                let ref_trip = ojp.fastest_trip().unwrap();

                SimplifiedTrip::try_from(ref_trip).unwrap()
            })
            .collect::<Vec<_>>();

        let hrdf_trips = ref_trips
            .iter()
            .map(|st| async move {
                let from_id = st.departure_id().expect("failed to get departure_id");
                let to_id = st.arrival_id().expect("failed to get arrival_id");
                let date_time =
                    utc_naive_to_local(st.departure_time().expect("failed to get departure_time"))
                        .with_second(0)
                        .unwrap();
                log::info!("Testing trip: {from_id} - {to_id} at {date_time}");
                plan_shortest_journey(hrdf, from_id, to_id, date_time, 10, false)
                    .as_ref()
                    .map(|r| STrip::from(r, hrdf).0)
            })
            .collect::<Vec<_>>();
        let hrdf_trips: Vec<_> = join_all(hrdf_trips).await;
        // We are only interested in the "failures" of the hrdf routing engine
        let failed_comparison = ref_trips
            .into_iter()
            .zip(hrdf_trips)
            .filter_map(|(rt, ht)| {
                if let Some(ht) = ht
                    && !rt.approx_equal(&ht, 0.1)
                {
                    Some((Some(rt), Some(ht)))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Ok(failed_comparison)
    }

    pub fn test_find_reachable_stops_within_time_limit(hrdf: &Hrdf) {
        let max_num_explorable_connections = 10;
        let mut departures = Vec::new();
        // 1. Petit-Lancy, Les Esserts (8587418)
        let departure_stop_id = 8587418;
        let departure_at = create_date_time(2025, 6, 1, 12, 30);
        departures.push((departure_stop_id, departure_at));

        // 2. Sevelen, Post (8588197)
        let departure_stop_id = 8588197;
        let departure_at = create_date_time(2025, 9, 2, 14, 2);
        departures.push((departure_stop_id, departure_at));

        // 3. Avully, village (8587031)
        let departure_stop_id = 8587031;
        let departure_at = create_date_time(2025, 7, 13, 16, 43);
        departures.push((departure_stop_id, departure_at));

        // 4. Bern, Bierhübeli (8590028)
        let departure_stop_id = 8590028;
        let departure_at = create_date_time(2025, 9, 17, 5, 59);
        departures.push((departure_stop_id, departure_at));

        // 5. Genève, gare Cornavin (8587057)
        let departure_stop_id = 8587057;
        let departure_at = create_date_time(2025, 10, 18, 20, 10);
        departures.push((departure_stop_id, departure_at));

        // 6. Villmergen, Zentrum (8587554)
        let departure_stop_id = 8587554;
        let departure_at = create_date_time(2025, 11, 22, 6, 59);
        departures.push((departure_stop_id, departure_at));

        // 7. Lugano, Genzana (8575310)
        let departure_stop_id = 8575310;
        let departure_at = create_date_time(2025, 4, 9, 8, 4);
        departures.push((departure_stop_id, departure_at));

        // 8. Zürich HB (8503000)
        let departure_stop_id = 8503000;
        let departure_at = create_date_time(2025, 6, 15, 12, 10);
        departures.push((departure_stop_id, departure_at));

        // 9. Campocologno (8509368)
        let departure_stop_id = 8509368;
        let departure_at = create_date_time(2025, 5, 29, 17, 29);
        departures.push((departure_stop_id, departure_at));

        // 10. Chancy, Douane (8587477)
        let departure_stop_id = 8587477;
        let departure_at = create_date_time(2025, 9, 10, 13, 37);
        departures.push((departure_stop_id, departure_at));

        let start_time = Instant::now();
        let time_limit = 60;
        for (departure_stop_id, departure_at) in departures.into_iter() {
            let coordinates = hrdf
                .data_storage()
                .stops()
                .data()
                .get(&departure_stop_id)
                .unwrap()
                .wgs84_coordinates()
                .unwrap();
            let routes = compute_routes_from_origin(
                hrdf,
                coordinates.latitude().unwrap(),
                coordinates.longitude().unwrap(),
                departure_at,
                Duration::minutes(time_limit),
                1,
                1,
                max_num_explorable_connections,
                false,
            );
            let mut data = unique_coordinates_from_routes(&routes, departure_at)
                .into_iter()
                .map(|(c, td)| {
                    (
                        c.easting().unwrap(),
                        c.northing().unwrap(),
                        td.num_minutes(),
                    )
                })
                .collect::<Vec<_>>();
            data.sort_by(|(la, lb, lc), (ra, rb, rc)| {
                let first = lc.cmp(rc);
                match first {
                    std::cmp::Ordering::Equal => {
                        let second = la.partial_cmp(ra).unwrap();
                        match second {
                            std::cmp::Ordering::Equal => lb.partial_cmp(rb).unwrap(),
                            _ => second,
                        }
                    }
                    _ => first,
                }
            });
            let fname = format!("test_json/ref_routes_{departure_stop_id}.json");
            eprintln!("Comparing {fname}");
            let reference = read_to_string(fname).unwrap();
            let (current, reference) = get_json_values(&data, &reference).unwrap();
            assert_eq!(current, reference);
        }

        println!("{:.2?}", start_time.elapsed());
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_journeys() {
        // First build hrdf file
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();
        let started = Instant::now();
        let failures = test_paths_validity(&hrdf, &IDS).await.unwrap();
        log::info!(
            "Time elapsed for all the HRDF tests: {:?}",
            started.elapsed()
        );
        for f in failures.iter() {
            if let (Some(ojp_trip), Some(hrdf_trip)) = f {
                eprintln!(
                    "{} - {}",
                    ojp_trip.departure_id().expect("failed to get departure_id"),
                    ojp_trip.arrival_id().expect("failed to get arrival_id")
                );
                eprintln!("OJP: \n{ojp_trip}");
                eprintln!("HRDF: \n{hrdf_trip}");
            }
        }
        assert!(failures.is_empty());
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_reachables_stops() {
        // First build hrdf file
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();
        let started = Instant::now();
        test_find_reachable_stops_within_time_limit(&hrdf);
        log::info!(
            "Time elapsed for all the HRDF tests: {:?}",
            started.elapsed()
        );
    }

    #[ignore]
    #[test(tokio::test)]
    async fn test_journeys_consistency() {
        // First build hrdf file
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();
        let started = Instant::now();
        let failures = test_paths_validity_consistency(&hrdf, &IDS).await.unwrap();
        log::info!(
            "Time elapsed for all the HRDF tests: {:?}",
            started.elapsed()
        );
        for f in failures.iter() {
            match f {
                (Some(forward_trip), Some(backward_trip)) => {
                    eprintln!(
                        "{} - {}",
                        forward_trip
                            .departure_id()
                            .expect("failed to get departure_id"),
                        forward_trip.arrival_id().expect("failed to get arrival_id")
                    );
                    eprintln!("FORWARD: \n{forward_trip}");
                    eprintln!("REVERSE: \n{backward_trip}");
                }
                (Some(forward_trip), None) => {
                    eprintln!(
                        "{} - {}",
                        forward_trip
                            .departure_id()
                            .expect("failed to get departure_id"),
                        forward_trip.arrival_id().expect("failed to get arrival_id")
                    );
                    eprintln!("FORWARD: \n{forward_trip}");
                    eprintln!("REVERSE: \nNONE FOUND");
                }
                (None, Some(backward_trip)) => {
                    eprintln!(
                        "{} - {}",
                        backward_trip
                            .departure_id()
                            .expect("failed to get departure_id"),
                        backward_trip
                            .arrival_id()
                            .expect("failed to get arrival_id")
                    );
                    eprintln!("FORWARD: \nNONE FOUND");
                    eprintln!("REVERSE: \n{backward_trip}");
                }
                _ => {}
            }
        }
        assert!(failures.is_empty());
    }

    /// Regression test for an infinite loop in `explore_routes`.
    /// Journey 333483 visits some stop several times, so `Route::extend` used to return a Route
    /// identical to the one just explored. That Route was pushed back in the queue and explored
    /// again forever. The origin below (hectare 60992524) reaches it from Himmelried,
    /// Schindelboden (8582811). The computation is done in another thread so that a regression
    /// fails the test instead of hanging the whole test suite.
    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_no_infinite_loop_when_journey_revisits_a_stop() {
        use std::{sync::Arc, sync::mpsc, time::Duration as StdDuration};

        let hrdf = Arc::new(Hrdf::try_from_year(2025, false, None).await.unwrap());
        let departure_at = create_date_time(2025, 4, 10, 7, 0);

        let (sender, receiver) = mpsc::channel();
        let hrdf_thread = Arc::clone(&hrdf);
        std::thread::spawn(move || {
            let routes = compute_routes_from_origin(
                &hrdf_thread,
                47.42233030307928,
                7.5698344995492715,
                departure_at,
                Duration::minutes(60),
                5,
                1,
                10,
                false,
            );
            let _ = sender.send(routes);
        });

        // The computation takes well under a second when the loop is not present.
        let routes = receiver
            .recv_timeout(StdDuration::from_secs(60))
            .expect("compute_routes_from_origin did not finish: infinite loop in explore_routes");
        assert!(!routes.is_empty());
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external polygon data"]
    async fn test_real_polygons_cache() {
        let original = ExcludedPolygons::try_new(
            &LAKES_GEOJSON_URLS,
            true,
            Some(env::temp_dir().to_string_lossy().to_string()),
        )
        .await
        .expect("Failed to create new polygons from online data");
        let loaded = ExcludedPolygons::try_new(
            &LAKES_GEOJSON_URLS,
            false,
            Some(env::temp_dir().to_string_lossy().to_string()),
        )
        .await
        .expect("Failed to create new polygons from cached");

        assert_eq!(original, loaded);
    }

    #[test(tokio::test)]
    #[cfg(feature = "hectare")]
    #[ignore = "requires downloading external hectare data"]
    async fn test_real_hectare_data_cache() {
        use std::env;

        let url = "https://dam-api.bfs.admin.ch/hub/api/dam/assets/32686751/master";
        let original = HectareData::new(
            url,
            true,
            Some(env::temp_dir().to_string_lossy().to_string()),
        )
        .await
        .expect("Failed to create new hectare data from online data");
        let loaded = HectareData::new(
            url,
            false,
            Some(env::temp_dir().to_string_lossy().to_string()),
        )
        .await
        .expect("Failed to create new polygons from cached");

        assert_eq!(original, loaded);
    }

    fn consistency_check(
        hrdf: &Hrdf,
        date_time: NaiveDateTime,
        dep_stop: i32,
        arr_stop: i32,
        num_connections: i32,
    ) {
        let forward_route = plan_journey(hrdf, dep_stop, arr_stop, date_time, 5, true).unwrap();
        let arrival_time = forward_route.arrival_at();
        println!(
            "Forward Found {dep_stop} -> {arr_stop}: Dep {:?} -> Arr {:?}",
            forward_route.departure_at(),
            arrival_time
        );

        println!(
            "Testing Reverse {dep_stop} -> {arr_stop}: arriving by {:?}",
            arrival_time
        );
        let reverse_route = plan_journey_reverse(
            hrdf,
            dep_stop,
            arr_stop,
            arrival_time,
            num_connections,
            true,
        )
        .unwrap();
        println!(
            "Reverse Found: Dep {:?} -> Arr {:?}",
            reverse_route.departure_at(),
            reverse_route.arrival_at()
        );

        assert!(
            reverse_route.departure_at() >= forward_route.departure_at(),
            "Reverse departure {:?} should be >= Forward departure {:?}",
            reverse_route.departure_at(),
            forward_route.departure_at()
        );
        assert!(
            reverse_route.arrival_at() == arrival_time,
            "Reverse arrival {:?} should be == Requested arrival {:?}",
            reverse_route.arrival_at(),
            arrival_time
        );
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_reverse_journey_bellinzona_zurich() {
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();
        // Use Swiss local time directly, without the OJP conversion or display layer.
        let departure_at = create_date_time(2025, 11, 25, 7, 23);
        let forward = plan_journey(&hrdf, 8583005, 8591046, departure_at, 11, false).unwrap();
        let reverse =
            plan_journey_reverse(&hrdf, 8583005, 8591046, forward.arrival_at(), 11, false).unwrap();

        assert!(reverse.arrival_at() <= forward.arrival_at());
        assert!(
            reverse.departure_at() >= forward.departure_at(),
            "Reverse departure {} is earlier than the known feasible departure {}",
            reverse.departure_at(),
            forward.departure_at(),
        );
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_reverse_journey_consistency() {
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();

        // Case 1: Simple direct trip
        // Zürich HB (8503000) -> Bern (8507000)
        let dep_stop = 8503000;
        let arr_stop = 8507000;
        let dep_time = create_date_time(2025, 6, 15, 10, 0); // 10:00
        println!("Testing Forward: Zürich -> Bern @ 10:00");
        consistency_check(&hrdf, dep_time, dep_stop, arr_stop, 10);

        // Case 2: Trip with Transfer
        // Zürich HB (8503000) -> Zermatt (8501689)
        let arr_stop = 8501689;
        println!("Testing Forward: Zürich -> Zermatt @ 08:00");
        let dep_time = create_date_time(2025, 6, 15, 8, 0);
        consistency_check(&hrdf, dep_time, dep_stop, arr_stop, 10);

        // Case 3: Trip with Transfer
        // Lausanne (8501120) -> Lugano, Vignola (8579006)
        let dep_stop = 8501120;
        let arr_stop = 8579006;
        println!("Testing Forward: Lausanne -> Lugano, Vignola @ 05:40");
        let dep_time = create_date_time(2025, 11, 25, 5, 40);
        consistency_check(&hrdf, dep_time, dep_stop, arr_stop, 10);

        // Case 4: Trip with Transfers
        // Thun, Schönau (8591921) -> Fribourg, Beaumont (8589143)
        // Trip from:  to:  departing at: 2025-11-24 15:15:00
        let dep_stop = 8591921;
        let arr_stop = 8589143;
        println!("Testing Forward: Thun, Schönau -> Fribourg, Beaumont @ 15:15");
        let dep_time = create_date_time(2025, 11, 24, 15, 15);
        consistency_check(&hrdf, dep_time, dep_stop, arr_stop, 10);

        // Case 5: Trip with Transfers
        // Thun, Schönau (8591921) -> Fribourg, Beaumont (8589143)
        // Trip from:  to:  departing at: 2025-11-24 15:15:00
        let dep_stop = 8509076;
        let arr_stop = 8587619;
        println!("Testing Forward: Davos Glaris -> Biel/Bienne, Place Guisan @ 06:50");
        let dep_time = create_date_time(2025, 11, 25, 6, 50);
        consistency_check(&hrdf, dep_time, dep_stop, arr_stop, 10);
    }

    /// Verifies that a forward isochrone from dep_stop contains arr_stop
    /// when given enough time to cover the known journey.
    fn forward_isochrone_contains_journey_destination(
        hrdf: &Hrdf,
        dep_stop: i32,
        arr_stop: i32,
        departure_at: NaiveDateTime,
    ) {
        let route = plan_journey(hrdf, dep_stop, arr_stop, departure_at, 10, false)
            .unwrap_or_else(|| panic!("Forward journey {dep_stop} -> {arr_stop} should exist"));
        let travel_time = route.arrival_at() - departure_at;

        eprintln!(
            "Forward isochrone check: {dep_stop} -> {arr_stop}, travel_time = {} min",
            travel_time.num_minutes()
        );

        let reachable = find_reachable_stops_within_time_limit(
            hrdf,
            dep_stop,
            departure_at,
            travel_time,
            10,
            false,
        );

        let reachable_stop_ids: std::collections::HashSet<i32> = reachable
            .iter()
            .filter_map(|r| r.arrival_stop_id())
            .collect();

        assert!(
            reachable_stop_ids.contains(&arr_stop),
            "Forward isochrone from {dep_stop} (limit {} min) should contain {arr_stop}. Found {} reachable stops.",
            travel_time.num_minutes(),
            reachable_stop_ids.len(),
        );
    }

    /// Verifies that a reverse isochrone to arr_stop contains dep_stop
    /// when given enough time to cover the known reverse journey.
    fn reverse_isochrone_contains_journey_origin(
        hrdf: &Hrdf,
        dep_stop: i32,
        arr_stop: i32,
        arrival_at: NaiveDateTime,
    ) {
        let route = plan_journey_reverse(hrdf, dep_stop, arr_stop, arrival_at, 10, false)
            .unwrap_or_else(|| panic!("Reverse journey {dep_stop} -> {arr_stop} should exist"));
        let travel_time = arrival_at - route.departure_at();

        eprintln!(
            "Reverse isochrone check: {dep_stop} -> {arr_stop}, travel_time = {} min",
            travel_time.num_minutes()
        );

        let origins =
            find_origin_stops_within_time_limit(hrdf, arr_stop, arrival_at, travel_time, 10, false);

        let origin_stop_ids: std::collections::HashSet<i32> = origins
            .iter()
            .filter_map(|r| r.departure_stop_id())
            .collect();

        assert!(
            origin_stop_ids.contains(&dep_stop),
            "Reverse isochrone to {arr_stop} (limit {} min) should contain {dep_stop}. Found {} origin stops.",
            travel_time.num_minutes(),
            origin_stop_ids.len(),
        );
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_forward_isochrone_contains_known_destinations() {
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();

        // Case 1: Zürich HB -> Bern (direct)
        eprintln!("Case 1: Zürich HB -> Bern");
        forward_isochrone_contains_journey_destination(
            &hrdf,
            8503000,
            8507000,
            create_date_time(2025, 6, 15, 10, 0),
        );

        // Case 2: Zürich HB -> Zermatt (with transfer)
        eprintln!("Case 2: Zürich HB -> Zermatt");
        forward_isochrone_contains_journey_destination(
            &hrdf,
            8503000,
            8501689,
            create_date_time(2025, 6, 15, 8, 0),
        );

        // Case 3: Lausanne -> Lugano, Vignola (with transfers)
        eprintln!("Case 3: Lausanne -> Lugano, Vignola");
        forward_isochrone_contains_journey_destination(
            &hrdf,
            8501120,
            8579006,
            create_date_time(2025, 11, 25, 5, 40),
        );
    }

    #[test(tokio::test)]
    #[ignore = "requires downloading external HRDF data"]
    async fn test_reverse_isochrone_contains_known_origins() {
        let hrdf = Hrdf::try_from_year(2025, false, None).await.unwrap();

        // For reverse, we first find the forward arrival time, then use it as the reverse target.

        // Case 1: Zürich HB -> Bern (direct)
        eprintln!("Case 1: Reverse to Bern");
        let departure_at = create_date_time(2025, 6, 15, 10, 0);
        let forward = plan_journey(&hrdf, 8503000, 8507000, departure_at, 10, false).unwrap();
        reverse_isochrone_contains_journey_origin(&hrdf, 8503000, 8507000, forward.arrival_at());

        // Case 2: Zürich HB -> Zermatt (with transfer)
        eprintln!("Case 2: Reverse to Zermatt");
        let departure_at = create_date_time(2025, 6, 15, 8, 0);
        let forward = plan_journey(&hrdf, 8503000, 8501689, departure_at, 10, false).unwrap();
        reverse_isochrone_contains_journey_origin(&hrdf, 8503000, 8501689, forward.arrival_at());

        // Case 3: Lausanne -> Lugano, Vignola (with transfers)
        eprintln!("Case 3: Reverse to Lugano, Vignola");
        let departure_at = create_date_time(2025, 11, 25, 5, 40);
        let forward = plan_journey(&hrdf, 8501120, 8579006, departure_at, 10, false).unwrap();
        reverse_isochrone_contains_journey_origin(&hrdf, 8501120, 8579006, forward.arrival_at());
    }
}
