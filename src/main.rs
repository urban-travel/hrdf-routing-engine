use std::{error::Error, sync::Arc};

use chrono::Duration;
use clap::Parser;
use futures::future::join_all;
use geo::MultiPolygon;
use hrdf_parser::{Loader, Timetable};
use hrdf_routing_engine::{
    Cli, IsochroneArgs, IsochroneDisplayMode, Mode, Pick, Router, lakes, plan_journey,
    plan_journey_reverse, run_comparison, run_debug, run_isochrones, run_service,
};
use log::LevelFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level("hrdf_routing_engine", LevelFilter::Info)
        .env()
        .init()?;

    let cli = Cli::parse();
    let cache = Cache {
        force_rebuild: cli.force_rebuild,
        dir: cli.cache_prefix.clone(),
    };
    let lakes = lakes(cli.force_rebuild, cli.cache_prefix).await?;
    let threads = cli.num_threads;
    let reverse = |args: IsochroneArgs| IsochroneArgs {
        reverse: true,
        ..args
    };
    let circles = IsochroneDisplayMode::Circles;

    match cli.mode {
        Mode::Serve { address, ports } => {
            let router = Arc::new(cache.router(Timetable::year(2026)).await?);
            let services = ports.into_iter().map(|port| {
                run_service(Arc::clone(&router), threads, lakes.clone(), address, port)
            });
            join_all(services).await;
        }
        Mode::Debug => run_debug(&cache.router(Timetable::year(2025)).await?),
        Mode::Journey(a) => {
            let router = cache.router(Timetable::date(a.at.date())).await?;
            let (from, to, max) = (
                a.departure_stop_id,
                a.arrival_stop_id,
                a.max_num_explorable_connections,
            );
            plan_journey(&router, from, to, a.at, max, a.verbose).ok_or("No journey found")?;
        }
        Mode::ReverseJourney(a) => {
            let router = cache.router(Timetable::date(a.at.date())).await?;
            let (from, to, max) = (
                a.departure_stop_id,
                a.arrival_stop_id,
                a.max_num_explorable_connections,
            );
            plan_journey_reverse(&router, from, to, a.at, max, a.verbose)
                .ok_or("No journey found")?;
        }
        Mode::Simple {
            isochrone_args: args,
            display,
        } => {
            cache
                .isochrones(
                    &lakes,
                    args,
                    Pick::Simple,
                    Duration::zero(),
                    display.mode,
                    threads,
                )
                .await?
        }
        Mode::ReverseSimple {
            isochrone_args: args,
            display,
        } => {
            cache
                .isochrones(
                    &lakes,
                    reverse(args),
                    Pick::Simple,
                    Duration::zero(),
                    display.mode,
                    threads,
                )
                .await?
        }
        Mode::Optimal(i) => {
            cache
                .isochrones(
                    &lakes,
                    i.isochrone_args,
                    Pick::Optimal,
                    i.window.delta_time,
                    i.display.mode,
                    threads,
                )
                .await?
        }
        Mode::Worst(i) => {
            cache
                .isochrones(
                    &lakes,
                    i.isochrone_args,
                    Pick::Worst,
                    i.window.delta_time,
                    i.display.mode,
                    threads,
                )
                .await?
        }
        Mode::ReverseOptimal(i) => {
            let args = reverse(i.isochrone_args);
            cache
                .isochrones(
                    &lakes,
                    args,
                    Pick::Optimal,
                    i.window.delta_time,
                    i.display.mode,
                    threads,
                )
                .await?
        }
        Mode::Average {
            isochrone_args: args,
            window,
        } => {
            cache
                .isochrones(
                    &lakes,
                    args,
                    Pick::Average,
                    window.delta_time,
                    circles,
                    threads,
                )
                .await?
        }
        Mode::ReverseAverage {
            isochrone_args: args,
            window,
        } => {
            cache
                .isochrones(
                    &lakes,
                    reverse(args),
                    Pick::Average,
                    window.delta_time,
                    circles,
                    threads,
                )
                .await?
        }
        Mode::Compare {
            isochrone_args: new,
            old_departure_at,
            window,
            display,
        } => {
            let old = IsochroneArgs {
                at: old_departure_at,
                ..new.clone()
            };
            let old_router = cache.router(Timetable::date(old.at.date())).await?;
            let new_router = cache.router(Timetable::date(new.at.date())).await?;
            run_comparison(
                &old_router,
                &new_router,
                &lakes,
                &old,
                &new,
                window.delta_time,
                display.mode,
                threads,
            )?;
        }
        #[cfg(feature = "hectare")]
        Mode::Hectare(args) => {
            let records = hrdf_routing_engine::hectare_records(
                &args.url,
                cache.force_rebuild,
                cache.dir.clone(),
            )
            .await?;
            let router = cache
                .router(Timetable::date(args.departure_at.date()))
                .await?;
            let records =
                hrdf_routing_engine::run_surface_per_ha(&router, &lakes, records, &args, threads);
            let path = format!("hectare_{}_{}.json", args.departure_at, args.time_limit);
            std::fs::write(path, serde_json::to_string_pretty(&records)?)?;
        }
    }
    Ok(())
}

struct Cache {
    force_rebuild: bool,
    dir: Option<String>,
}

impl Cache {
    async fn router(&self, loader: Loader) -> Result<Router, hrdf_parser::Error> {
        let loader = loader.force_rebuild(self.force_rebuild);
        let loader = match &self.dir {
            Some(dir) => loader.cache_dir(dir),
            None => loader,
        };
        Ok(Router::new(loader.load().await?))
    }

    /// Loads the timetable of the isochrones' date, then computes and draws them.
    async fn isochrones(
        &self,
        lakes: &MultiPolygon,
        args: IsochroneArgs,
        pick: Pick,
        delta_time: Duration,
        display_mode: IsochroneDisplayMode,
        num_threads: usize,
    ) -> Result<(), Box<dyn Error>> {
        let router = self.router(Timetable::date(args.at.date())).await?;
        Ok(run_isochrones(
            &router,
            lakes,
            &args,
            pick,
            delta_time,
            display_mode,
            num_threads,
        )?)
    }
}
