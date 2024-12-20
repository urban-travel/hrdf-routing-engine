mod debug;
mod isochrone;
mod routing;
mod service;
mod utils;

pub use isochrone::compute_isochrones;
pub use routing::find_reachable_stops_within_time_limit;
pub use routing::plan_journey;
pub use routing::Route;
pub use routing::RouteSection;

use std::{env, error::Error};

use debug::run_debug;
use hrdf_parser::{Hrdf, Version};
use service::run_service;

pub async fn run() -> Result<(), Box<dyn Error>> {
    let hrdf = Hrdf::new(
        Version::V_5_40_41_2_0_5,
        "https://opentransportdata.swiss/en/dataset/timetable-54-2024-hrdf/permalink",
        false,
    )
    .await?;

    let args: Vec<String> = env::args().collect();

    if args.get(1).map(|s| s.as_str()) == Some("serve") {
        run_service(hrdf).await;
    } else {
        run_debug(hrdf);
    }

    Ok(())
}
