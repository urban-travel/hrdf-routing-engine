use std::{error::Error, net::Ipv4Addr};

use clap::{Parser, Subcommand};
use hrdf_parser::{Hrdf, Version};
use hrdf_routing_engine::IsochroneDisplayMode;
use hrdf_routing_engine::{run_debug, run_service, run_test};
use log::LevelFilter;

#[derive(Subcommand)]
enum Mode {
    /// Serve mode to a given port
    Serve {
        /// Tpv4 served, defaults to 0.0.0.0
        #[arg(short, long, default_value_t = Ipv4Addr::new(0, 0, 0, 0))]
        address: Ipv4Addr,

        /// Port exposed on the server
        #[arg(short, long, default_value_t = 8100)]
        port: u16,
    },
    /// Debug mode used to check if the examples still run
    Debug,
    /// Test new features
    Test {
        /// Display mode of the isochrones: circles or contour_line
        #[arg(short, long)]
        mode: IsochroneDisplayMode,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// prefix path for the cache, when absent defaults lo "./"
    #[arg(short, long)]
    cache_prefix: Option<String>,

    /// What mode is used
    #[command(subcommand)]
    mode: Mode,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level("hrdf_routing_engine", LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let cli = Cli::parse();

    let hrdf = Hrdf::new(
        Version::V_5_40_41_2_0_7,
        "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
        false,
        cli.cache_prefix,
    )
    .await?;

    match cli.mode {
        Mode::Debug => {
            run_debug(hrdf);
        }
        Mode::Serve { address, port } => {
            run_service(hrdf, address, port).await;
        }
        Mode::Test { mode } => {
            run_test(hrdf, mode)?;
        }
    }

    Ok(())
}
