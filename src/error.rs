use std::num::{ParseFloatError, ParseIntError};

use thiserror::Error;
#[cfg(feature = "hectare")]
use zip::result::ZipError;

#[derive(Debug, Error)]
pub enum RError {
    #[error("No bounding rectangle exists")]
    NoBoundingRect,
    #[error("Io Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("GeoJsonError: {0}")]
    GeoJsonError(Box<geojson::Error>),
    #[error("Postcard error: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("Failed to parse integer: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Failed to parse float: {0}")]
    ParseFloatError(#[from] ParseFloatError),
    #[cfg(feature = "hectare")]
    #[error("Csv error: {0}")]
    CsvError(#[from] csv::Error),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[cfg(feature = "hectare")]
    #[error("Failed decompress data: {0}")]
    Decompress(#[from] ZipError),
}

impl From<geojson::Error> for RError {
    fn from(value: geojson::Error) -> Self {
        Self::GeoJsonError(Box::new(value))
    }
}

pub type RResult<T> = Result<T, RError>;
