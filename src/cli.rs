use std::net::Ipv4Addr;

use chrono::{Duration, NaiveDateTime};
use clap::{Parser, Subcommand};

use crate::{IsochroneArgs, IsochroneDisplayMode};

pub(crate) fn date_time(s: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
}

pub(crate) fn minutes(s: &str) -> Result<Duration, std::num::ParseIntError> {
    s.parse().map(Duration::minutes)
}

#[derive(clap::Args, Debug, Clone)]
pub struct JourneyArgs {
    /// Departure stop id
    #[arg(long, default_value_t = 8587418)]
    pub departure_stop_id: i32,
    /// Arrival stop id
    #[arg(long, default_value_t = 8595120)]
    pub arrival_stop_id: i32,
    /// Departure date and time (arrival for reverse-journey)
    #[arg(short = 'd', long = "departure-at", visible_alias = "arrival-at", short_alias = 'a', value_parser = date_time, default_value = "2025-09-17 17:05:59")]
    pub at: NaiveDateTime,
    /// Maximum number of transfers
    #[arg(short, long, default_value_t = 10)]
    pub max_num_explorable_connections: i32,
    /// Verbose on or off
    #[arg(short, long)]
    pub verbose: bool,
}

#[cfg(feature = "hectare")]
#[derive(clap::Args, Debug, Clone)]
pub struct HectareArgs {
    /// Departure date and time
    #[arg(short, long, value_parser = date_time, default_value = "2025-04-10 07:30:00")]
    pub departure_at: NaiveDateTime,
    /// Maximum time of the isochrone in minutes
    #[arg(short, long, value_parser = minutes, default_value = "60")]
    pub time_limit: Duration,
    /// Maximum number of transfers
    #[arg(short, long, default_value_t = 10)]
    pub max_num_explorable_connections: i32,
    /// Number of stops within walking distance to start from
    #[arg(short, long, default_value_t = 5)]
    pub num_starting_points: usize,
    /// Verbose on or off
    #[arg(short, long)]
    pub verbose: bool,
    /// The +/- duration over which to look for the optimal isochrone (in minutes)
    #[arg(long, value_parser = minutes, default_value = "30")]
    pub delta_time: Duration,
    /// Where to download the population per hectare from
    #[arg(short, long, default_value_t = String::from("https://dam-api.bfs.admin.ch/hub/api/dam/assets/32686751/master"))]
    pub url: String,
}

#[derive(Subcommand)]
pub enum Mode {
    /// Serve mode to a given port
    Serve {
        /// IPv4 address served
        #[arg(short, long, default_value_t = Ipv4Addr::new(0, 0, 0, 0))]
        address: Ipv4Addr,
        /// Ports exposed by the server
        #[arg(short, long, num_args = 1.., default_values_t = [8100u16])]
        ports: Vec<u16>,
    },
    /// Debug mode used to check if the examples still run
    Debug,
    /// Journey mode to find a journey between two stop ids departing at a given time
    Journey(JourneyArgs),
    /// Reverse Journey mode to find latest departure
    ReverseJourney(JourneyArgs),
    /// Compare between two years for the optimal isochrone for a given duration
    Compare {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        /// Second departure date and time
        #[arg(short, long, value_parser = date_time, default_value = "2025-04-11 15:36:00")]
        old_departure_at: NaiveDateTime,
        #[command(flatten)]
        window: Window,
        #[command(flatten)]
        display: Display,
    },
    /// Compute the optimal isochrones (largest surface)
    Optimal(Isochrones),
    /// Compute the worst isochrones (smallest surface)
    Worst(Isochrones),
    /// Single isochrone at a specific location and date-time
    Simple {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        #[command(flatten)]
        display: Display,
    },
    /// Average surface isochrone given a specific location, date-time, and duration
    Average {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        #[command(flatten)]
        window: Window,
    },
    /// Reverse isochrone: find all origins from which a destination can be reached
    ReverseSimple {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        #[command(flatten)]
        display: Display,
    },
    /// Reverse optimal isochrone (largest surface)
    ReverseOptimal(Isochrones),
    /// Reverse average isochrone
    ReverseAverage {
        #[command(flatten)]
        isochrone_args: IsochroneArgs,
        #[command(flatten)]
        window: Window,
    },
    /// Surface per Hectare
    #[cfg(feature = "hectare")]
    Hectare(HectareArgs),
}

/// Isochrones picked in a time window.
#[derive(clap::Args)]
pub struct Isochrones {
    #[command(flatten)]
    pub isochrone_args: IsochroneArgs,
    #[command(flatten)]
    pub window: Window,
    #[command(flatten)]
    pub display: Display,
}

#[derive(clap::Args)]
pub struct Window {
    /// The +/- duration over which to compute (in minutes)
    #[arg(long, value_parser = minutes, default_value = "30")]
    pub delta_time: Duration,
}

#[derive(clap::Args)]
pub struct Display {
    /// Display mode of the isochrones
    #[arg(long, value_enum, default_value = "circles")]
    pub mode: IsochroneDisplayMode,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Directory of the caches (default: current directory)
    #[arg(short, long)]
    pub cache_prefix: Option<String>,
    /// Force to rebuild the cache
    #[arg(short, long, default_value_t = false)]
    pub force_rebuild: bool,
    /// Maximum number of cores used. If 0 is given all cores are automatically assigned
    #[arg(long, default_value_t = 4)]
    pub num_threads: usize,
    /// What mode is used
    #[command(subcommand)]
    pub mode: Mode,
}
