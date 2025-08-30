
Public transport routing engine based on Swiss HRDF data.

Author: Florian Burgener
Contributors:

* Marc Gay-Balmaz
* Orestis Malaspinas

[https://crates.io/crates/hrdf-routing-engine](https://crates.io/crates/hrdf-routing-engine)

## Prerequisites

* Rust Toolchain (https://www.rust-lang.org/tools/install)
* OpenSSL (`apt install libssl-dev` on Ubuntu)

## Installation

```sh
git clone https://github.com/florianburgener/hrdf-routing-engine
cd hrdf-routing-engine
```

## Usage

There are many modes of use of this library, but currently the main one is the computation of isochrones.

It can be used in server mode or as a standalone program.

The CLI has therefore several modes: `serve`, `debug`, `compare`, `optimal`, `worst`, `simple`, and `average`

```console
$ cargo run -- --help
Public transport routing engine based on Swiss HRDF data.

Usage: hrdf-routing-engine [OPTIONS] <COMMAND>

Commands:
  serve    Serve mode to a given port
  debug    Debug mode used to check if the examples still run
  compare  Compare between two years
  optimal  Compute the optimal isochrones
  worst    Compute the optimal isochrones
  simple   Simple isochrone
  average  Average isochrone
```

Each of the modes has a separate use as described below.

### Serve

Launches a server that can be used with the [isochrone frontend](https://github.com/florianburgener/interactive-isochrone-map).

### Debug

Runs several examples to test if they are still running.

### Simple

Computes a single isochrone from a specified position at a given time for a given maximum time limit and interval between isochrones.
The isochrones can be shown as polygons (`circles` display mode) or as isocontours (`contour_line` display mode).

```console
$ cargo run --release -- simple --help
Simple isochrone

Usage: hrdf-routing-engine simple [OPTIONS]

Options:
      --latitude <LATITUDE>
          Departure latitude [default: 46.20956654]
      --longitude <LONGITUDE>
          Departure longitude [default: 6.13536]
  -d, --departure-at <DEPARTURE_AT>
          Departure date and time [default: "2025-04-10 15:36:00"]
  -t, --time-limit <TIME_LIMIT>
          Maximum time of the isochrone in minutes [default: 60]
  -i, --interval <INTERVAL>
          Time interval between two isochrone in minutes [default: 10]
  -m, --max-num-explorable-connections <MAX_NUM_EXPLORABLE_CONNECTIONS>
          Maximum number of connections [default: 10]
  -n, --num-starting-points <NUM_STARTING_POINTS>
          Number of starting points [default: 5]
  -v, --verbose
          Verbose on or off
      --mode <MODE>
          Display mode of the isochrones: circles or contour_line [default: circles]
```

There are three configuration options:

* `--num-starting-points`: how many starting stops should we investigate
* `--max-num-explorable-connections`: how many exchanges are admitted
* `--verbose` shows more debug informations

### Optimal

Computes the optimal isochrone given a departure date and time and from a specific location.

```console
$ cargo run -- optimal --help                                                                                                    Sun 27 Jul 2025 12:19:42 PM EEST
Compute the optimal isochrones

Usage: hrdf-routing-engine optimal [OPTIONS]

Options:
      --latitude <LATITUDE>
          Departure latitude [default: 46.20956654]
      --longitude <LONGITUDE>
          Departure longitude [default: 6.13536]
  -d, --departure-at <DEPARTURE_AT>
          Departure date and time [default: "2025-04-10 15:36:00"]
  -t, --time-limit <TIME_LIMIT>
          Maximum time of the isochrone in minutes [default: 60]
  -i, --interval <INTERVAL>
          Time interval between two isochrone in minutes [default: 10]
  -m, --max-num-explorable-connections <MAX_NUM_EXPLORABLE_CONNECTIONS>
          Maximum number of connections [default: 10]
  -n, --num-starting-points <NUM_STARTING_POINTS>
          Number of starting points [default: 5]
  -v, --verbose
          Verbose on or off
      --delta-time <DELTA_TIME>
          The +/- duration on which to compute the average (in minutes) [default: 30]
      --mode <MODE>
          Display mode of the isochrones: circles or contour_line [default: circles]
  -h, --help
          Print help
```

The optimality is determined by the largest surface atainable in the span of `[departure-at - delta-time, departure-at + delta-time)`, during a certain duration.
The surface is computed for every minute in the time interval, and only the largest is retained.

### Average

Computes the average surface at a given location

```console
$ cargo run -- average --help                                                                                            389ms  Sun 27 Jul 2025 12:19:44 PM EEST
Average surface isochrone given a specific location, date-time, and duration

Usage: hrdf-routing-engine average [OPTIONS]

Options:
      --latitude <LATITUDE>
          Departure latitude [default: 46.20956654]
      --longitude <LONGITUDE>
          Departure longitude [default: 6.13536]
  -d, --departure-at <DEPARTURE_AT>
          Departure date and time [default: "2025-04-10 15:36:00"]
  -t, --time-limit <TIME_LIMIT>
          Maximum time of the isochrone in minutes [default: 60]
  -i, --interval <INTERVAL>
          Time interval between two isochrone in minutes [default: 10]
  -m, --max-num-explorable-connections <MAX_NUM_EXPLORABLE_CONNECTIONS>
          Maximum number of connections [default: 10]
  -n, --num-starting-points <NUM_STARTING_POINTS>
          Number of starting points [default: 5]
  -v, --verbose
          Verbose on or off
      --delta-time <DELTA_TIME>
          The +/- duration on which to compute the average (in minutes) [default: 30]
  -h, --help
          Print help
```

The average is computed in the span of `[departure-at - delta-time, departure-at + delta-time)`.

### Compare

Compares the optimal isochrone for two given date-times for a given location.

```console
$ cargo run -- compare --help                                                                                            246ms  Sun 27 Jul 2025 12:33:22 PM EEST
Compare between two years for the optimal isochrone for a given duration

Usage: hrdf-routing-engine compare [OPTIONS]

Options:
      --latitude <LATITUDE>
          Departure latitude [default: 46.20956654]
      --longitude <LONGITUDE>
          Departure longitude [default: 6.13536]
  -d, --departure-at <DEPARTURE_AT>
          Departure date and time [default: "2025-04-10 15:36:00"]
  -t, --time-limit <TIME_LIMIT>
          Maximum time of the isochrone in minutes [default: 60]
  -i, --interval <INTERVAL>
          Time interval between two isochrone in minutes [default: 10]
  -m, --max-num-explorable-connections <MAX_NUM_EXPLORABLE_CONNECTIONS>
          Maximum number of connections [default: 10]
  -n, --num-starting-points <NUM_STARTING_POINTS>
          Number of starting points [default: 5]
  -v, --verbose
          Verbose on or off
  -o, --old-departure-at <OLD_DEPARTURE_AT>
          Second departure date and time [default: "2024-04-11 15:36:00"]
      --mode <MODE>
          Display mode of the isochrones: circles or contour_line [default: circles]
      --delta-time <DELTA_TIME>
          The +/- duration on which to compute the average (in minutes) [default: 30]
  -h, --help
          Print help
```

For each date-time, the optimal isochrone is computed in the span of `[departure-at - delta-time, departure-at + delta-time)`.

