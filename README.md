# HRDF Routing Engine

Public transport routing engine based on Swiss HRDF data.

Author: Florian Burgener

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

Starts the routing engine in debug mode:
```sh
cargo run --release
```

Starts the routing engine in web service mode (port 8100):
```sh
cargo run --release -- serve
```
