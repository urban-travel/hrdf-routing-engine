use std::{error::Error, net::Ipv4Addr};

use chrono::{Duration, NaiveDateTime};
use clap::{Parser, Subcommand};
use hrdf_parser::{Hrdf, Version};
use hrdf_routing_engine::{
    IsochroneDisplayMode, run_average, run_comparison, run_debug, run_optimal, run_service,
    run_simple, run_worst,
};
use log::LevelFilter;

#[derive(Parser, Debug)]
struct IsochroneArgs {
    /// Departure latitude
    #[arg(long, default_value_t = 46.20956654)]
    latitude: f64,
    /// Departure longitude
    #[arg(long, default_value_t = 6.13536000)]
    longitude: f64,
    /// Departure date and time
    #[arg(short, long, default_value_t = String::from("2025-04-10 15:36:00"))]
    departure_at: String,
    /// Maximum time of the isochrone in minutes
    #[arg(short, long, default_value_t = 60)]
    time_limit: i64,
    /// Time interval between two isochrone in minutes
    #[arg(short, long, default_value_t = 10)]
    interval: i64,
    /// Verbose on or off
    #[arg(short, long, default_value_t = true)]
    verbose: bool,
}

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
    /// Compare between two years
    Compare {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        /// Departure date and time
        #[arg(short, long, default_value_t = String::from("2024-04-11 15:36:00"))]
        departure_at_old: String,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(short, long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(short, long, default_value_t = 30)]
        delta_time: i64,
    },
    /// Compute the optimal isochrones
    Optimal {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(short, long, default_value_t = 30)]
        delta_time: i64,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(short, long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
    },
    /// Compute the optimal isochrones
    Worst {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(short, long, default_value_t = 30)]
        delta_time: i64,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(short, long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
    },
    /// Simple isochrone
    Simple {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(short, long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
    },
    /// Average isochrone
    Average {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(short, long, default_value_t = 30)]
        delta_time: i64,
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

    let hrdf_2025 = Hrdf::new(
        Version::V_5_40_41_2_0_7,
        "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
        false,
        cli.cache_prefix.clone(),
    )
    .await?;

    match cli.mode {
        Mode::Debug => {
            run_debug(hrdf_2025);
        }
        Mode::Serve { address, port } => {
            run_service(hrdf_2025, address, port).await;
        }
        Mode::Optimal {
            isochrone_args,
            delta_time,
            mode,
        } => {
            let IsochroneArgs {
                latitude,
                longitude,
                departure_at,
                time_limit,
                interval,
                verbose,
            } = isochrone_args;
            run_optimal(
                hrdf_2025,
                longitude,
                latitude,
                NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
                Duration::minutes(time_limit),
                Duration::minutes(interval),
                Duration::minutes(delta_time),
                mode,
                verbose,
            )?;
        }
        Mode::Worst {
            isochrone_args,
            delta_time,
            mode,
        } => {
            let IsochroneArgs {
                latitude,
                longitude,
                departure_at,
                time_limit,
                interval,
                verbose,
            } = isochrone_args;
            run_worst(
                hrdf_2025,
                longitude,
                latitude,
                NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
                Duration::minutes(time_limit),
                Duration::minutes(interval),
                Duration::minutes(delta_time),
                mode,
                verbose,
            )?;
        }
        Mode::Simple {
            isochrone_args,
            mode,
        } => {
            let IsochroneArgs {
                latitude,
                longitude,
                departure_at,
                time_limit,
                interval,
                verbose,
            } = isochrone_args;

            run_simple(
                hrdf_2025,
                longitude,
                latitude,
                NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
                Duration::minutes(time_limit),
                Duration::minutes(interval),
                mode,
                verbose,
            )?;
        }
        Mode::Average {
            isochrone_args,
            delta_time,
        } => {
            let IsochroneArgs {
                latitude,
                longitude,
                departure_at,
                time_limit,
                interval,
                verbose,
            } = isochrone_args;

            run_average(
                hrdf_2025,
                longitude,
                latitude,
                NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
                Duration::minutes(time_limit),
                Duration::minutes(interval),
                Duration::minutes(delta_time),
                verbose,
            )?;
        }
        Mode::Compare {
            isochrone_args,
            mode,
            departure_at_old,
            delta_time,
        } => {
            let IsochroneArgs {
                latitude,
                longitude,
                departure_at,
                time_limit,
                interval,
                verbose,
            } = isochrone_args;
            let hrdf_2024 = Hrdf::new(
                Version::V_5_40_41_2_0_7,
                "https://data.opentransportdata.swiss/en/dataset/timetable-54-2024-hrdf/permalink",
                false,
                cli.cache_prefix,
            )
            .await?;
            run_comparison(
                hrdf_2024,
                hrdf_2025,
                longitude,
                latitude,
                NaiveDateTime::parse_from_str(&departure_at_old, "%Y-%m-%d %H:%M:%S")?,
                NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
                Duration::minutes(time_limit),
                Duration::minutes(interval),
                Duration::minutes(delta_time),
                mode,
                verbose,
            )?;
        }
    }

    Ok(())
}
