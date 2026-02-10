use futures::future::join_all;
use hrdf_routing_engine::{Cli, Mode};
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use chrono::Duration;
use clap::Parser;
use hrdf_parser::Hrdf;
use hrdf_routing_engine::{
    ExcludedPolygons, LAKES_GEOJSON_URLS, plan_journey, run_average, run_comparison, run_debug,
    run_optimal, run_service, run_simple, run_worst,
};
#[cfg(feature = "hectare")]
use hrdf_routing_engine::{HectareData, run_surface_per_ha};
use log::LevelFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level("hrdf_routing_engine", LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let cli = Cli::parse();

    let excluded_polygons = ExcludedPolygons::try_new(
        &LAKES_GEOJSON_URLS,
        cli.force_rebuild,
        cli.cache_prefix.clone(),
    )
    .await?;

    match cli.mode {
        Mode::Debug => {
            let hrdf =
                Hrdf::try_from_year(2025, cli.force_rebuild, cli.cache_prefix.clone()).await?;
            run_debug(hrdf);
        }
        Mode::Journey { journey_args } => {
            let journey_args = journey_args.finalize()?;
            let hrdf = Hrdf::try_from_date(
                journey_args.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;

            let _ = plan_journey(
                &hrdf,
                journey_args.departure_stop_id,
                journey_args.arrival_stop_id,
                journey_args.departure_at,
                journey_args.max_num_explorable_connections,
                journey_args.verbose,
            )
            .unwrap_or_else(|| panic!("Error: no journey found for {journey_args}"));
        }
        Mode::Serve { address, ports } => {
            let hrdf_2026 =
                Hrdf::try_from_year(2026, cli.force_rebuild, cli.cache_prefix.clone()).await?;
            let ahrdf = Arc::new(hrdf_2026);
            let services: Vec<_> = ports
                .into_iter()
                .map(|p| {
                    let value = excluded_polygons.clone();
                    let hrdf = Arc::clone(&ahrdf);
                    async move {
                        run_service(hrdf, cli.num_threads, value, address, p).await;
                    }
                })
                .collect();
            join_all(services).await;
        }
        Mode::Optimal {
            isochrone_args,
            delta_time,
            mode,
        } => {
            let isochrone_args = isochrone_args.finalize()?;
            let hrdf = Hrdf::try_from_date(
                isochrone_args.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;
            run_optimal(
                hrdf,
                excluded_polygons,
                isochrone_args,
                Duration::minutes(delta_time),
                mode,
                cli.num_threads,
            )?;
        }
        Mode::Worst {
            isochrone_args,
            delta_time,
            mode,
        } => {
            let isochrone_args = isochrone_args.finalize()?;
            let hrdf = Hrdf::try_from_date(
                isochrone_args.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;
            run_worst(
                hrdf,
                excluded_polygons,
                isochrone_args,
                Duration::minutes(delta_time),
                mode,
                cli.num_threads,
            )?;
        }
        Mode::Simple {
            isochrone_args,
            mode,
        } => {
            let isochrone_args = isochrone_args.finalize()?;
            let hrdf = Hrdf::try_from_date(
                isochrone_args.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;
            run_simple(
                hrdf,
                excluded_polygons,
                isochrone_args,
                mode,
                cli.num_threads,
            )?;
        }
        Mode::Average {
            isochrone_args,
            delta_time,
        } => {
            let isochrone_args = isochrone_args.finalize()?;
            let hrdf_2026 = Hrdf::try_from_date(
                isochrone_args.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;
            run_average(
                hrdf_2026,
                excluded_polygons,
                isochrone_args,
                Duration::minutes(delta_time),
                cli.num_threads,
            )?;
        }
        Mode::Compare {
            isochrone_args,
            mode,
            old_departure_at,
            delta_time,
        } => {
            let args_new = isochrone_args.clone().finalize()?;
            let args_old = isochrone_args
                .set_departure_at(old_departure_at)
                .finalize()?;

            let hrdf_old = Hrdf::try_from_date(
                args_old.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;
            let hrdf_new = Hrdf::try_from_date(
                args_new.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix,
            )
            .await?;
            run_comparison(
                hrdf_old,
                hrdf_new,
                excluded_polygons,
                args_old,
                args_new,
                Duration::minutes(delta_time),
                mode,
                cli.num_threads,
            )?;
        }

        #[cfg(feature = "hectare")]
        Mode::Hectare {
            isochrone_args,
            delta_time,
            url,
        } => {
            use hrdf_routing_engine::IsochroneDisplayMode;

            let isochrone_args = isochrone_args.finalize()?;
            let hectare =
                HectareData::new(&url, cli.force_rebuild, cli.cache_prefix.clone()).await?;
            let hrdf_2026 = Hrdf::try_from_date(
                isochrone_args.departure_at.date(),
                cli.force_rebuild,
                cli.cache_prefix.clone(),
            )
            .await?;
            let surfaces = run_surface_per_ha(
                hrdf_2026,
                excluded_polygons,
                hectare,
                isochrone_args.clone(),
                Duration::minutes(delta_time),
                IsochroneDisplayMode::Circles,
                cli.num_threads,
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
