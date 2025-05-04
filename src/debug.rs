use std::time::Instant;

use chrono::Duration;
use hrdf_parser::Hrdf;

use crate::{
    routing::{find_reachable_stops_within_time_limit, plan_journey},
    utils::create_date_time,
};

pub fn run_debug(hrdf: Hrdf) {
    println!();
    println!(
        "------------------------------------------------------------------------------------------------"
    );
    println!("--- Debug");
    println!(
        "------------------------------------------------------------------------------------------------"
    );

    test_plan_journey(&hrdf);
    test_find_reachable_stops_within_time_limit(&hrdf);
}

#[allow(dead_code)]
pub fn test_plan_journey(hrdf: &Hrdf) {
    // ------------------------------------------------------------------------------------------------
    // --- 2.0.5
    // ------------------------------------------------------------------------------------------------

    let verbose = true;

    println!();
    let start_time = Instant::now();

    // 1. Petit-Lancy, Les Esserts => Onex, Bandol
    plan_journey(
        hrdf,
        8587418,
        8593027,
        create_date_time(2025, 6, 1, 12, 30),
        verbose,
    );

    // 2. Petit-Lancy, Les Esserts => Genève-Aéroport
    plan_journey(
        hrdf,
        8587418,
        8501026,
        create_date_time(2025, 2, 9, 14, 2),
        verbose,
    );

    // 3. Avully, village => Pont-Céard, gare
    plan_journey(
        hrdf,
        8587031,
        8593189,
        create_date_time(2025, 7, 13, 16, 43),
        verbose,
    );

    // 4. Petit-Lancy, Les Esserts => Vevey, Palud
    plan_journey(
        hrdf,
        8587418,
        8595120,
        create_date_time(2025, 9, 17, 5, 59),
        verbose,
    );

    // 5. Genève, gare Cornavin => Avusy, village
    plan_journey(
        hrdf,
        8587057,
        8587032,
        create_date_time(2025, 10, 18, 20, 10),
        verbose,
    );

    // 6. Genève => Bern, Bierhübeli
    plan_journey(
        hrdf,
        8501008,
        8590028,
        create_date_time(2025, 11, 22, 6, 59),
        verbose,
    );

    // 7. Genève => Zürich HB
    plan_journey(
        hrdf,
        8501008,
        8503000,
        create_date_time(2025, 4, 9, 8, 4),
        verbose,
    );

    // 8. Zürich HB => Lugano, Genzana
    plan_journey(
        hrdf,
        8503000,
        8575310,
        create_date_time(2025, 6, 15, 12, 10),
        verbose,
    );

    // 9. Chancy, Douane => Campocologno
    plan_journey(
        hrdf,
        8587477,
        8509368,
        create_date_time(2025, 5, 29, 17, 29),
        verbose,
    );

    // 10. Chancy, Douane => Sevelen, Post
    plan_journey(
        hrdf,
        8587477,
        8581989,
        create_date_time(2025, 9, 10, 13, 37),
        true,
    );

    //11. Genève => Paris gare de Lyon
    plan_journey(
        hrdf,
        8501008,
        8768600,
        create_date_time(2025, 4, 28, 8, 29),
        true,
    );

    //12. Genève => Lausanne
    plan_journey(
        hrdf,
        8501008,
        8501120,
        create_date_time(2025, 4, 28, 8, 20),
        true,
    );

    println!("\n{:.2?}", start_time.elapsed());
}

#[allow(dead_code)]
pub fn test_find_reachable_stops_within_time_limit(hrdf: &Hrdf) {
    // 1. Petit-Lancy, Les Esserts (8587418)
    let departure_stop_id = 8587418;
    let departure_at = create_date_time(2025, 6, 1, 12, 30);

    // 2. Sevelen, Post (8588197)
    // let departure_stop_id = 8588197;
    // let departure_at = create_date_time(2025, 9, 2, 14, 2);

    // 3. Avully, village (8587031)
    // let departure_stop_id = 8587031;
    // let departure_at = create_date_time(2025, 7, 13, 16, 43);

    // 4. Bern, Bierhübeli (8590028)
    // let departure_stop_id = 8590028;
    // let departure_at = create_date_time(2025, 9, 17, 5, 59);

    // 5. Genève, gare Cornavin (8587057)
    // let departure_stop_id = 8587057;
    // let departure_at = create_date_time(2025, 10, 18, 20, 10);

    // 6. Villmergen, Zentrum (8587554)
    // let departure_stop_id = 8587554;
    // let departure_at = create_date_time(2025, 11, 22, 6, 59);

    // 7. Lugano, Genzana (8575310)
    // let departure_stop_id = 8575310;
    // let departure_at = create_date_time(2025, 4, 9, 8, 4);

    // 8. Zürich HB (8503000)
    // let departure_stop_id = 8503000;
    // let departure_at = create_date_time(2025, 6, 15, 12, 10);

    // 9. Campocologno (8509368)
    // let departure_stop_id = 8509368;
    // let departure_at = create_date_time(2025, 5, 29, 17, 29);

    // 10. Chancy, Douane (8587477)
    // let departure_stop_id = 8587477;
    // let departure_at = create_date_time(2025, 9, 10, 13, 37);

    let start_time = Instant::now();
    for time_limit in [60, 120, 180] {
        let routes = find_reachable_stops_within_time_limit(
            hrdf,
            departure_stop_id,
            departure_at,
            Duration::minutes(time_limit),
            false,
        );

        println!("\n{}", routes.len());
    }

    println!("{:.2?}", start_time.elapsed());
}
#[cfg(test)]
mod tests {
    use crate::debug::{test_find_reachable_stops_within_time_limit, test_plan_journey};

    use hrdf_parser::{Hrdf, Version};
    use test_log::test;

    #[test(tokio::test)]
    async fn debug() {
        let hrdf = Hrdf::new(
            Version::V_5_40_41_2_0_7,
            "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
            false,
            None,
        )
        .await
        .unwrap();

        test_plan_journey(&hrdf);
        test_find_reachable_stops_within_time_limit(&hrdf);
    }
}
