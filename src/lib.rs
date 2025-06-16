mod routing;
mod utils;

use routing::plan_journey;
use utils::create_date_time;

use std::error::Error;
use std::time::Instant;

use hrdf_parser::{Hrdf, Version};

use crate::routing::{AlgorithmArgs, RoutingData};

pub async fn run() -> Result<(), Box<dyn Error>> {
    let hrdf = Hrdf::new(
        Version::V_5_40_41_2_0_5,
        "https://data.opentransportdata.swiss/dataset/8646c29f-f562-45f3-a559-731cc5cb4368/resource/954dd1bf-a424-4608-bb53-2b2f5f619521/download/oev_sammlung_ch_hrdf_5_40_41_2024_20240902_221428.zip",
        false,
    )
    .await?;

    test_plan_journey(&hrdf);

    Ok(())
}

#[allow(dead_code)]
#[rustfmt::skip]
fn test_plan_journey(hrdf: &Hrdf) {
    // ------------------------------------------------------------------------------------------------
    // --- 2.0.5
    // ------------------------------------------------------------------------------------------------
    const N: u32 = 40;

    let routing_data = RoutingData::new(hrdf.data_storage());

    println!();
    let start_time = Instant::now();
    let mut journey = None;

    for i in 0..N {
        let verbose = i == 0;

        // 1. Petit-Lancy, Les Esserts => Onex, Bandol
        // journey = Some(plan_journey(AlgorithmArgs::new(&routing_data, 8587418, 8593027, create_date_time(2024, 6, 1, 12, 30), verbose)));

        // 2. Petit-Lancy, Les Esserts => Genève-Aéroport
        // plan_journey(AlgorithmArgs::new(&routing_data, 8587418, 8501026, create_date_time(2024, 2, 9, 14, 2), verbose));

        // 3. Avully, village => Pont-Céard, gare
        // plan_journey(AlgorithmArgs::new(&routing_data, 8587031, 8593189, create_date_time(2024, 7, 13, 16, 43), verbose));

        // 4. Petit-Lancy, Les Esserts => Vevey, Palud
        // plan_journey(AlgorithmArgs::new(&routing_data, 8587418, 8595120, create_date_time(2024, 9, 17, 5, 59), verbose));

        // 5. Genève, gare Cornavin => Avusy, village
        // plan_journey(AlgorithmArgs::new(&routing_data, 8587057, 8587032, create_date_time(2024, 10, 18, 20, 10), verbose));

        // 6. Genève => Bern, Bierhübeli
        // plan_journey(AlgorithmArgs::new(&routing_data, 8501008, 8590028, create_date_time(2024, 11, 22, 6, 59), verbose));

        // 7. Genève => Zürich HB
        // journey = Some(plan_journey(AlgorithmArgs::new(&routing_data, 8501008, 8503000, create_date_time(2024, 4, 9, 8, 4), verbose)));

        // 8. Zürich HB => Lugano, Genzana
        // journey = Some(plan_journey(AlgorithmArgs::new(&    routing_data, 8503000, 8575310, create_date_time(2024, 6, 15, 12, 10), verbose)));

        // 9. Chancy, Douane => Campocologno
        // plan_journey(AlgorithmArgs::new(&routing_data, 8587477, 8509368, create_date_time(2024, 5, 29, 17, 29), verbose));

        // 10. Chancy, Douane => Sevelen, Post
        journey = Some(plan_journey(AlgorithmArgs::new(&routing_data, 8587477, 8588197, create_date_time(2024, 9, 10, 13, 37), verbose)));
    }

    println!("\n{:.2?}\n", start_time.elapsed() / N);
    journey.unwrap().print(routing_data.data_storage());
}
