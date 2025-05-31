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
        "https://data.opentransportdata.swiss/dataset/8646c29f-f562-45f3-a559-731cc5cb4368/resource/954dd1bf-a424-4608-bb53-2b2f5f619521/download/oev_sammlung_ch_hrdf_5_40_41_2024_20240902_221428.zip",
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
