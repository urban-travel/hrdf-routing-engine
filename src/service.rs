use std::{
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use axum::{extract::Query, http::StatusCode, routing::get, Json, Router};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use hrdf_parser::{timetable_end_date, timetable_start_date, Hrdf};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::isochrone::{self, IsochroneDisplayMode, IsochroneMap};

pub async fn run_service(hrdf: Hrdf, ip_addr: Ipv4Addr, port: u16) {
    log::info!("Starting the server...");

    let hrdf = Arc::new(hrdf);
    let hrdf_1 = Arc::clone(&hrdf);
    let hrdf_2 = Arc::clone(&hrdf);
    let cors = CorsLayer::new().allow_methods(Any).allow_origin(Any);

    #[rustfmt::skip]
    let app = Router::new()
        .route(
            "/metadata",
            get(move || metadata(Arc::clone(&hrdf_1))),
        )
        .route(
            "/isochrones",
            get(move |params| compute_isochrones(Arc::clone(&hrdf_2), params)),
        )
        .layer(cors);
    let address = SocketAddr::from((ip_addr, port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();

    log::info!("Listening on {ip_addr}:{port}...");

    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Serialize)]
struct MetadataResponse {
    start_date: NaiveDate,
    end_date: NaiveDate,
}

async fn metadata(hrdf: Arc<Hrdf>) -> Json<MetadataResponse> {
    Json(MetadataResponse {
        start_date: timetable_start_date(hrdf.data_storage().timetable_metadata()).unwrap(),
        end_date: timetable_end_date(hrdf.data_storage().timetable_metadata()).unwrap(),
    })
}

#[derive(Debug, Deserialize)]
struct ComputeIsochronesRequest {
    origin_point_latitude: f64,
    origin_point_longitude: f64,
    departure_date: NaiveDate,
    departure_time: NaiveTime,
    time_limit: u32,
    isochrone_interval: u32,
    display_mode: String,
}

async fn compute_isochrones(
    hrdf: Arc<Hrdf>,
    Query(params): Query<ComputeIsochronesRequest>,
) -> Result<Json<IsochroneMap>, StatusCode> {
    // The coordinates are not checked but should be.

    let start_date = timetable_start_date(hrdf.data_storage().timetable_metadata()).unwrap();
    let end_date = timetable_end_date(hrdf.data_storage().timetable_metadata()).unwrap();

    if params.departure_date < start_date || params.departure_date > end_date {
        // The departure date is outside the possible dates for the timetable.
        return Err(StatusCode::BAD_REQUEST);
    }

    if params.time_limit % params.isochrone_interval != 0 {
        // The result of dividing time_limit with isochrone_interval must be an integer.
        return Err(StatusCode::BAD_REQUEST);
    }

    if !["circles", "contour_line"].contains(&params.display_mode.as_str()) {
        // The display mode is incorrect.
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = isochrone::compute_isochrones(
        &hrdf,
        params.origin_point_latitude,
        params.origin_point_longitude,
        NaiveDateTime::new(params.departure_date, params.departure_time),
        Duration::minutes(params.time_limit.into()),
        Duration::minutes(params.isochrone_interval.into()),
        IsochroneDisplayMode::from_str(&params.display_mode).unwrap(),
        false,
    );
    Ok(Json(result))
}
