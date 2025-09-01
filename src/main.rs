use std::fs::File;
use std::io::Write;
use std::{error::Error, net::Ipv4Addr};

use chrono::{Duration, NaiveDateTime};
use clap::{Parser, Subcommand};
use hrdf_parser::{Hrdf, Version};
use hrdf_routing_engine::{
    ExcludedPolygons, IsochroneArgs, IsochroneDisplayMode, LAKES_GEOJSON_URLS, run_average,
    run_comparison, run_debug, run_optimal, run_service, run_simple, run_worst,
};
#[cfg(feature = "hectare")]
use hrdf_routing_engine::{HectareData, IsochroneHectareArgs, run_surface_per_ha};
use log::LevelFilter;

#[derive(Parser, Debug, Clone)]
struct IsochroneArgsBuilder {
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
    /// Maximum number of connections
    #[arg(short, long, default_value_t = 10)]
    max_num_explorable_connections: i32,
    /// Number of starting points
    #[arg(short, long, default_value_t = 5)]
    num_starting_points: usize,
    /// Verbose on or off
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

impl IsochroneArgsBuilder {
    pub(crate) fn set_departure_at(mut self, departure_at: String) -> Self {
        self.departure_at = departure_at;
        self
    }

    pub(crate) fn finalize(self) -> Result<IsochroneArgs, Box<dyn Error>> {
        let Self {
            latitude,
            longitude,
            departure_at,
            time_limit,
            interval,
            max_num_explorable_connections,
            num_starting_points,
            verbose,
        } = self;

        Ok(IsochroneArgs {
            latitude,
            longitude,
            departure_at: NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
            time_limit: Duration::minutes(time_limit),
            interval: Duration::minutes(interval),
            max_num_explorable_connections,
            num_starting_points,
            verbose,
        })
    }
}

#[cfg(feature = "hectare")]
#[derive(Parser, Debug)]
struct IsochroneHectareArgsBuilder {
    /// Departure date and time
    #[arg(short, long, default_value_t = String::from("2025-04-10 07:30:00"))]
    departure_at: String,
    /// Maximum time of the isochrone in minutes
    #[arg(short, long, default_value_t = 60)]
    time_limit: i64,
    /// Maximum number of connections
    #[arg(short, long, default_value_t = 10)]
    max_num_explorable_connections: i32,
    /// Number of starting points
    #[arg(short, long, default_value_t = 5)]
    num_starting_points: usize,
    /// Verbose on or off
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[cfg(feature = "hectare")]
impl IsochroneHectareArgsBuilder {
    pub(crate) fn finalize(self) -> Result<IsochroneHectareArgs, Box<dyn Error>> {
        let Self {
            departure_at,
            time_limit,
            max_num_explorable_connections,
            num_starting_points,
            verbose,
        } = self;

        Ok(IsochroneHectareArgs {
            departure_at: NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
            time_limit: Duration::minutes(time_limit),
            max_num_explorable_connections,
            num_starting_points,
            verbose,
        })
    }
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
    /// Compare between two years for the optimal isochrone for a given duration
    Compare {
        #[command(flatten)]
        isochrone_args: IsochroneArgsBuilder,
        /// Second departure date and time
        #[arg(short, long, default_value_t = String::from("2024-04-11 15:36:00"))]
        old_departure_at: String,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(long, default_value_t = 30)]
        delta_time: i64,
    },
    /// Compute the optimal isochrones (largest surface)
    Optimal {
        #[command(flatten)]
        isochrone_args: IsochroneArgsBuilder,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(long, default_value_t = 30)]
        delta_time: i64,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
    },
    /// Compute the worst isochrones (smallest surface)
    Worst {
        #[command(flatten)]
        isochrone_args: IsochroneArgsBuilder,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(long, default_value_t = 30)]
        delta_time: i64,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
    },
    /// Single isochrone at a specific location and date-time
    Simple {
        #[command(flatten)]
        isochrone_args: IsochroneArgsBuilder,
        /// Display mode of the isochrones: circles or contour_line
        #[arg(long, default_value_t = IsochroneDisplayMode::Circles)]
        mode: IsochroneDisplayMode,
    },
    /// Average surface isochrone given a specific location, date-time, and duration
    Average {
        #[command(flatten)]
        isochrone_args: IsochroneArgsBuilder,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(long, default_value_t = 30)]
        delta_time: i64,
    },
    /// Surface per Hectare
    #[cfg(feature = "hectare")]
    Hectare {
        #[command(flatten)]
        isochrone_args: IsochroneHectareArgsBuilder,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(long, default_value_t = 30)]
        delta_time: i64,
        /// The +/- duration on which to compute the average (in minutes)
        #[arg(short, long, default_value_t = String::from("https://dam-api.bfs.admin.ch/hub/api/dam/assets/32686751/master"))]
        url: String,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Prefix path for the cache, when absent defaults lo "./"
    #[arg(short, long)]
    cache_prefix: Option<String>,
    /// Force to rebuild the cache
    #[arg(short, long, default_value_t = false)]
    force_rebuild: bool,
    /// What mode is used
    #[command(subcommand)]
    mode: Mode,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level("hrdf_routing_engine", LevelFilter::Debug)
        .env()
        .init()
        .unwrap();
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    let cli = Cli::parse();

    let hrdf_2025 = Hrdf::new(
        Version::V_5_40_41_2_0_7,
        "https://data.opentransportdata.swiss/en/dataset/timetable-54-2025-hrdf/permalink",
        cli.force_rebuild,
        cli.cache_prefix.clone(),
    )
    .await?;

    let excluded_polygons = ExcludedPolygons::try_new(
        &LAKES_GEOJSON_URLS,
        cli.force_rebuild,
        cli.cache_prefix.clone(),
    )
    .await?;

    match cli.mode {
        Mode::Debug => {
            run_debug(hrdf_2025);
        }
        Mode::Serve { address, port } => {
            run_service(hrdf_2025, excluded_polygons, address, port).await;
        }
        Mode::Optimal {
            isochrone_args,
            delta_time,
            mode,
        } => {
            run_optimal(
                hrdf_2025,
                excluded_polygons,
                isochrone_args.finalize()?,
                Duration::minutes(delta_time),
                mode,
            )?;
        }
        Mode::Worst {
            isochrone_args,
            delta_time,
            mode,
        } => {
            run_worst(
                hrdf_2025,
                excluded_polygons,
                isochrone_args.finalize()?,
                Duration::minutes(delta_time),
                mode,
            )?;
        }
        Mode::Simple {
            isochrone_args,
            mode,
        } => {
            run_simple(
                hrdf_2025,
                excluded_polygons,
                isochrone_args.finalize()?,
                mode,
            )?;
        }
        Mode::Average {
            isochrone_args,
            delta_time,
        } => {
            run_average(
                hrdf_2025,
                excluded_polygons,
                isochrone_args.finalize()?,
                Duration::minutes(delta_time),
            )?;
        }
        Mode::Compare {
            isochrone_args,
            mode,
            old_departure_at,
            delta_time,
        } => {
            let args_2025 = isochrone_args.clone().finalize()?;
            let args_2024 = isochrone_args
                .set_departure_at(old_departure_at)
                .finalize()?;

            let hrdf_2024 = Hrdf::new(
                Version::V_5_40_41_2_0_7,
                "https://data.opentransportdata.swiss/en/dataset/timetable-54-2024-hrdf/permalink",
                cli.force_rebuild,
                cli.cache_prefix,
            )
            .await?;
            run_comparison(
                hrdf_2024,
                hrdf_2025,
                excluded_polygons,
                args_2024,
                args_2025,
                Duration::minutes(delta_time),
                mode,
            )?;
        }

        #[cfg(feature = "hectare")]
        Mode::Hectare {
            isochrone_args,
            delta_time,
            url,
        } => {
            let isochrone_args = isochrone_args.finalize()?;
            let hectare =
                HectareData::new(&url, cli.force_rebuild, cli.cache_prefix.clone()).await?;
            let surfaces = run_surface_per_ha(
                hrdf_2025,
                excluded_polygons,
                hectare,
                isochrone_args.clone(),
                Duration::minutes(delta_time),
                IsochroneDisplayMode::Circles,
            )?;

            let data = serde_json::to_string_pretty(&surfaces).unwrap();
            let fname = format!(
                "hectare_{}_{}.json",
                isochrone_args.departure_at, isochrone_args.time_limit
            );
            let mut f = File::create(&fname).expect("Unable to create file");
            f.write_all(data.as_bytes()).expect("Unable to write data");
        }
    }

    Ok(())
}
