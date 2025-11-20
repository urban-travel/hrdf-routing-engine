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

    use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};
    use hrdf_parser::{Hrdf, Version};
    use ojp_rs::{OJP, SimplifiedLeg, SimplifiedTrip};
    use rand::prelude::IndexedRandom;

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

    pub async fn test_paths_validity(
        hrdf: &Hrdf,
        test_cities: &[&str],
        date_time: NaiveDateTime,
    ) -> Result<Vec<(Option<SimplifiedTrip>, Option<SimplifiedTrip>)>, Box<dyn Error>> {
        dotenvy::dotenv().ok(); // optional
        let number_results = 10;
        let point_ref =
            OJP::find_locations(test_cities, date_time, number_results, "OJP-HRDF", "TOKEN")
                .await?;

        let num_travels = 20;
        let points = point_ref
            .choose_multiple(&mut rand::rng(), 2 * num_travels)
            .copied()
            .collect::<Vec<_>>();
        let (departures, arrivals) = points.split_at(num_travels);
        let number_results = 3;
        let ref_trips = OJP::find_trips(
            departures,
            arrivals,
            date_time,
            number_results,
            "OJP-HRDF",
            "TOKEN",
        )
        .await;
        let hrdf_trips = departures
            .iter()
            .zip(arrivals.iter())
            .map(|(&from_id, &to_id)| async move {
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
            .filter_map(|(rt, ht)| match (rt, ht) {
                (Ok(rt), Some(ht)) => {
                    if !rt.approx_equal(&ht, 0.1) {
                        Some((Some(rt), Some(ht)))
                    } else {
                        None
                    }
                }
                (Ok(rt), None) => Some((Some(rt), None)),
                (Err(_), Some(_)) => None,
                (Err(_), None) => None,
            })
            .collect::<Vec<_>>();

        Ok(failed_comparison)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_journeys() {
        let test_cities = [
            "Zürich",
            "Genève",
            "Basel",
            "Lausanne",
            "Bern",
            "Winterthur",
            "Lucerne",
            "St. Gallen",
            "Lugano",
            "Biel",
            "Thun",
            "Bellinzona",
            "Fribourg",
            "Schaffhausen",
            "Chur",
            "Sion",
            "Zug",
            "Glaris",
        ];
        let year = 2025;
        let month = 11;
        let day = 1;
        let hour = 11;
        let min = 16;
        let sec = 17;
        let date_time = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(year, month, day).unwrap(),
            NaiveTime::from_hms_opt(hour, min, sec).unwrap(),
        );
        // First build hrdf file
        let hrdf = Hrdf::new(
            Version::V_5_40_41_2_0_7,
            "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
            false,
            None,
        )
        .await
        .unwrap();
        let failures = test_paths_validity(&hrdf, &test_cities, date_time)
            .await
            .unwrap();
        eprintln!("{:?}", failures);
    }
}
