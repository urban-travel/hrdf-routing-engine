use std::net::Ipv4Addr;

use chrono::{Duration, NaiveDateTime};
use clap::{Parser, Subcommand};

#[cfg(feature = "hectare")]
use crate::IsochroneHectareArgs;
use crate::{IsochroneArgs, IsochroneDisplayMode, JourneyArgs, RResult};

#[derive(Parser, Debug, Clone)]
pub struct IsochroneArgsBuilder {
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
    pub fn set_departure_at(mut self, departure_at: String) -> Self {
        self.departure_at = departure_at;
        self
    }

    pub fn finalize(self) -> RResult<IsochroneArgs> {
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

#[derive(Parser, Debug, Clone)]
pub struct JourneyArgsBuilder {
    /// Departure stop id
    #[arg(long, default_value_t = 8587418)]
    departure_stop_id: i32,
    /// Departure longitude
    #[arg(long, default_value_t = 8595120)]
    arrival_stop_id: i32,
    /// Departure date and time
    #[arg(short, long, default_value_t = String::from("2025-09-17 17:05:59"))]
    departure_at: String,
    /// Maximum number of connections
    #[arg(short, long, default_value_t = 10)]
    max_num_explorable_connections: i32,
    /// Verbose on or off
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

impl JourneyArgsBuilder {
    pub fn finalize(self) -> RResult<JourneyArgs> {
        let Self {
            departure_stop_id,
            arrival_stop_id,
            departure_at,
            max_num_explorable_connections,
            verbose,
        } = self;

        Ok(JourneyArgs {
            departure_stop_id,
            arrival_stop_id,
            departure_at: NaiveDateTime::parse_from_str(&departure_at, "%Y-%m-%d %H:%M:%S")?,
            max_num_explorable_connections,
            verbose,
        })
    }
}

#[cfg(feature = "hectare")]
#[derive(Parser, Debug)]
pub struct IsochroneHectareArgsBuilder {
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
    pub fn finalize(self) -> RResult<IsochroneHectareArgs> {
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
pub enum Mode {
    /// Serve mode to a given port
    Serve {
        /// Tpv4 served, defaults to 0.0.0.0
        #[arg(short, long, default_value_t = Ipv4Addr::new(0, 0, 0, 0))]
        address: Ipv4Addr,

        /// Port exposed on the server
        #[arg(short, long, value_parser = clap::value_parser!(u16), num_args = 1.., default_values_t = [8100u16])]
        ports: Vec<u16>,
    },
    /// Debug mode used to check if the examples still run
    Debug,
    /// Journey mode to find a journey between two stop ids departing at a given time
    Journey {
        #[command(flatten)]
        journey_args: JourneyArgsBuilder,
    },
    /// Compare between two years for the optimal isochrone for a given duration
    Compare {
        #[command(flatten)]
        isochrone_args: IsochroneArgsBuilder,
        /// Second departure date and time
        #[arg(short, long, default_value_t = String::from("2025-04-11 15:36:00"))]
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
        /// The URL from where to download the necessary data for the Hectare computations
        #[arg(short, long, default_value_t = String::from("https://dam-api.bfs.admin.ch/hub/api/dam/assets/32686751/master"))]
        url: String,
    },
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Prefix path for the cache, when absent defaults lo "./"
    #[arg(short, long)]
    pub cache_prefix: Option<String>,
    /// Force to rebuild the cache
    #[arg(short, long, default_value_t = false)]
    pub force_rebuild: bool,
    // Maximum number of cores used. If 0 is given all cores are automatically assigned
    #[arg(long, default_value_t = 4)]
    pub num_threads: usize,
    /// What mode is used
    #[command(subcommand)]
    pub mode: Mode,
}
