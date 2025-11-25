mod app;
mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

#[cfg(feature = "hectare")]
pub use app::run_surface_per_ha;
pub use app::{run_average, run_comparison, run_optimal, run_simple, run_worst};
pub use debug::run_debug;
pub use isochrone::externals::{ExcludedPolygons, LAKES_GEOJSON_URLS};
pub use isochrone::{IsochroneArgs, IsochroneDisplayMode};
#[cfg(feature = "hectare")]
pub use isochrone::{IsochroneHectareArgs, externals::HectareData};
pub use routing::{Route, plan_journey, plan_shortest_journey};
pub use service::run_service;

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::debug::{test_find_reachable_stops_within_time_limit, test_plan_journey};
    use chrono::{TimeDelta, Timelike};
    use hrdf_parser::{Hrdf, Version};
    use ojp_rs::{OJP, SimplifiedLeg, SimplifiedTrip};

    use test_log::test;

    use crate::{Route, plan_shortest_journey};
    use futures::future::join_all;

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
                        departure_time,
                        arrival_time,
                        format!("{:?}", s.transport()),
                    )
                })
                .collect::<Vec<_>>();
            STrip(SimplifiedTrip::new(legs))
        }
    }

    static IDS: [(i32, i32); 38] = [
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
        (8589616, 8502495),
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
        (8592588, 8506236),
        (8501120, 8579006),
        (8591418, 8592834),
        (8570732, 8573673),
        (8578997, 8576815),
        (8585206, 8506302),
        (8589587, 8592133),
        (22, 8592904),
        (8592889, 8589566),
        (8572453, 8591998),
        (8500236, 8511236),
        (8574226, 8583259),
        (8575155, 8500161),
    ];

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
                let from_id = st.departure_id();
                let to_id = st.arrival_id();
                let date_time = st.departure_time().with_second(0).unwrap();
                plan_shortest_journey(hrdf, from_id, to_id, date_time, 10, false)
                    .as_ref()
                    .map(|r| STrip::from(r, hrdf).0)
            })
            .collect::<Vec<_>>();
        let hrdf_trips: Vec<_> = join_all(hrdf_trips).await;
        // We are only interested in the "failures" of the hrdf routing engine
        let failed_comparison = ref_trips
            .into_iter()
            .zip(hrdf_trips.into_iter())
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

    #[test(tokio::test)]
    async fn test_journeys() {
        // First build hrdf file
        let hrdf = Hrdf::new(
            Version::V_5_40_41_2_0_7,
            "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
            false,
            None,
        )
        .await
        .unwrap();
        let failures = test_paths_validity(&hrdf, &IDS).await.unwrap();
        for f in failures.iter() {
            if let (Some(ojp_trip), Some(hrdf_trip)) = f {
                eprintln!("{} - {}", ojp_trip.departure_id(), ojp_trip.arrival_id());
                eprintln!("OJP: \n{ojp_trip}");
                eprintln!("HRDF: \n{hrdf_trip}");
            }
        }
        assert!(failures.is_empty());
        test_plan_journey(&hrdf);
        test_find_reachable_stops_within_time_limit(&hrdf);
    }
}
