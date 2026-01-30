use std::time::Instant;

use hrdf_parser::Hrdf;

use crate::{routing::plan_journey, utils::create_date_time};

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
}

#[allow(dead_code)]
pub fn test_plan_journey(hrdf: &Hrdf) {
    // ------------------------------------------------------------------------------------------------
    // --- 2.0.5
    // ------------------------------------------------------------------------------------------------

    let max_num_explorable_connections = 10;
    let verbose = true;

    println!();
    let start_time = Instant::now();

    // 1. Petit-Lancy, Les Esserts => Onex, Bandol
    println!("1. Petit-Lancy, Les Esserts => Onex, Bandol");
    plan_journey(
        hrdf,
        8587418,
        8593027,
        create_date_time(2025, 6, 1, 12, 30),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 2. Petit-Lancy, Les Esserts => Genève-Aéroport
    println!("2. Petit-Lancy, Les Esserts => Genève-Aéroport");
    plan_journey(
        hrdf,
        8587418,
        8501026,
        create_date_time(2025, 2, 9, 14, 2),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 3. Avully, village => Pont-Céard, gare
    println!("3. Avully, village => Pont-Céard, gare");
    plan_journey(
        hrdf,
        8587031,
        8593189,
        create_date_time(2025, 7, 13, 16, 43),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 4. Petit-Lancy, Les Esserts => Vevey, Palud
    println!("4. Petit-Lancy, Les Esserts => Vevey, Palud");
    plan_journey(
        hrdf,
        8587418,
        8595120,
        create_date_time(2025, 9, 17, 5, 59),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 5. Genève, gare Cornavin => Avusy, village
    println!("5. Genève, gare Cornavin => Avusy, village");
    plan_journey(
        hrdf,
        8587057,
        8587032,
        create_date_time(2025, 10, 18, 20, 10),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 6. Genève => Bern, Bierhübeli
    println!("6. Genève => Bern, Bierhübeli");
    plan_journey(
        hrdf,
        8501008,
        8590028,
        create_date_time(2025, 11, 22, 6, 59),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 7. Genève => Zürich HB
    println!("7. Genève => Zürich HB");
    plan_journey(
        hrdf,
        8501008,
        8503000,
        create_date_time(2025, 4, 9, 8, 4),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 8. Zürich HB => Lugano, Genzana
    println!("8. Zürich HB => Lugano, Genzana");
    plan_journey(
        hrdf,
        8503000,
        8575310,
        create_date_time(2025, 6, 15, 12, 10),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 9. Chancy, Douane => Campocologno
    println!("9. Chancy, Douane => Campocologno");
    plan_journey(
        hrdf,
        8587477,
        8509368,
        create_date_time(2025, 5, 29, 17, 29),
        max_num_explorable_connections,
        verbose,
    );
    println!("==============================================");

    // 10. Chancy, Douane => Sevelen, Post
    println!("10. Chancy, Douane => Sevelen, Post");
    plan_journey(
        hrdf,
        8587477,
        8581989,
        create_date_time(2025, 9, 10, 13, 37),
        max_num_explorable_connections,
        true,
    );
    println!("==============================================");

    // 11. Genève => Paris gare de Lyon
    println!("11. Genève => Paris gare de Lyon");
    plan_journey(
        hrdf,
        8501008,
        8768600,
        create_date_time(2025, 4, 28, 8, 29),
        max_num_explorable_connections,
        true,
    );
    println!("==============================================");

    //12. Genève => Lausanne
    println!("12. Genève => Lausanne");
    plan_journey(
        hrdf,
        8501008,
        8501120,
        create_date_time(2025, 4, 28, 8, 20),
        max_num_explorable_connections,
        true,
    );
    println!("==============================================");

    println!("\n{:.2?}", start_time.elapsed());
}
