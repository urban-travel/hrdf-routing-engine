use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use geo::MultiPolygon;
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    IsochroneArgs, Router,
    isochrone::{self, IsochroneDisplayMode, IsochroneMap},
    planner,
};

pub struct AppState {
    pub router: Arc<Router>,
    pub index: planner::Index,
    excluded_polygons: MultiPolygon,
    num_threads: usize,
}

pub type AppStateRef = State<Arc<AppState>>;

pub async fn run_service(
    router: Arc<Router>,
    num_threads: usize,
    excluded_polygons: MultiPolygon,
    ip_addr: Ipv4Addr,
    port: u16,
) {
    log::info!("Starting the server...");
    let index = planner::Index::new(&router.timetable);
    let state = Arc::new(AppState {
        router,
        index,
        excluded_polygons,
        num_threads,
    });
    let app = axum::Router::new()
        .route("/metadata", get(metadata))
        .route(
            "/isochrones",
            get(|state: AppStateRef, request| isochrones(state, request, false)),
        )
        .route(
            "/reverse-isochrones",
            get(|state: AppStateRef, request| isochrones(state, request, true)),
        )
        .route("/stops", get(planner::stops))
        .route("/journeys", get(planner::journeys))
        .layer(CorsLayer::new().allow_methods(Any).allow_origin(Any))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from((ip_addr, port)))
        .await
        .unwrap();
    log::info!("Listening on {ip_addr}:{port}...");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize)]
struct MetadataResponse {
    start_date: NaiveDate,
    end_date: NaiveDate,
}

async fn metadata(State(state): AppStateRef) -> Json<MetadataResponse> {
    let tt = &state.router.timetable;
    Json(MetadataResponse {
        start_date: tt.start,
        end_date: tt.end,
    })
}

/// The point and time are the origin and departure, or the destination and arrival for the
/// reverse isochrones (which also accept these names).
#[derive(Deserialize)]
struct IsochronesRequest {
    #[serde(alias = "destination_latitude")]
    origin_point_latitude: f64,
    #[serde(alias = "destination_longitude")]
    origin_point_longitude: f64,
    #[serde(alias = "arrival_date")]
    departure_date: NaiveDate,
    #[serde(alias = "arrival_time")]
    departure_time: NaiveTime,
    time_limit: u32,
    isochrone_interval: u32,
    display_mode: IsochroneDisplayMode,
    find_optimal: bool,
}

async fn isochrones(
    State(state): AppStateRef,
    Query(request): Query<IsochronesRequest>,
    reverse: bool,
) -> Result<Json<IsochroneMap>, StatusCode> {
    let tt = &state.router.timetable;
    let (time_limit, interval) = (request.time_limit, request.isochrone_interval);
    // The time limit must be a multiple of the interval.
    if !(tt.start..=tt.end).contains(&request.departure_date)
        || interval == 0
        || time_limit % interval != 0
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let args = IsochroneArgs {
        latitude: request.origin_point_latitude,
        longitude: request.origin_point_longitude,
        at: NaiveDateTime::new(request.departure_date, request.departure_time),
        time_limit: Duration::minutes(time_limit.into()),
        interval: Duration::minutes(interval.into()),
        max_num_explorable_connections: 10,
        num_starting_points: 5,
        verbose: false,
        reverse,
    };
    log::info!(
        "Computing isochrones for {args}, optimal: {}",
        request.find_optimal
    );
    let (router, excluded, threads, mode) = (
        &state.router,
        &state.excluded_polygons,
        state.num_threads,
        request.display_mode,
    );
    Ok(Json(if request.find_optimal {
        isochrone::compute_optimal_isochrones(
            router,
            excluded,
            &args,
            Duration::minutes(30),
            mode,
            threads,
        )
    } else {
        isochrone::compute_isochrones(router, excluded, &args, mode, threads)
    }))
}
