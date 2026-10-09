//! Data from elsewhere: the lakes isochrones leave out, and the population per hectare.

use std::{env, fs, io::Cursor, path::PathBuf};

use geo::{BooleanOps, MultiPolygon, Polygon};
use geojson::{FeatureCollection, GeoJson};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::RResult;

const LAKES_URL: &str = "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-";
const LAKES: [&str; 20] = [
    "baldegg",
    "biel",
    "brienz",
    "constance",
    "geneva",
    "hallwil",
    "lac-de-joux",
    "lucerne",
    "lugano",
    "maggiore",
    "morat",
    "neuchatel",
    "of-gruyere",
    "sarnen",
    "sempach",
    "sihl",
    "thun",
    "wagitalersee",
    "walensee",
    "zurich",
];

/// The Swiss lakes, in WGS84 (latitude, longitude).
pub async fn lakes(force_rebuild: bool, cache_prefix: Option<String>) -> RResult<MultiPolygon> {
    let urls: Vec<_> = LAKES
        .iter()
        .map(|lake| format!("{LAKES_URL}{lake}.geojson"))
        .collect();
    let build = async {
        let mut lakes = MultiPolygon::new(vec![]);
        for url in &urls {
            let path = download(url, "").await?;
            log::info!("Parsing lakes from {}...", path.display());
            lakes = lakes.union(&parse_geojson(&fs::read_to_string(path)?)?);
        }
        Ok(lakes)
    };
    cached(
        &cache_path(cache_prefix, &urls.concat()),
        force_rebuild,
        build,
    )
    .await
}

/// The polygons of a GeoJSON feature collection.
fn parse_geojson(geojson: &str) -> RResult<MultiPolygon> {
    let polygons = FeatureCollection::try_from(geojson.parse::<GeoJson>()?)?
        .into_iter()
        .filter_map(|feature| match feature.geometry?.value {
            geojson::GeometryValue::Polygon { coordinates } => Some(coordinates),
            _ => None,
        })
        .map(|rings| {
            // GeoJSON is (longitude, latitude).
            let polygon = |ring: Vec<geojson::Position>| {
                Polygon::new(ring.into_iter().map(|c| (c[1], c[0])).collect(), vec![])
            };
            rings.into_iter().map(polygon).collect::<MultiPolygon>()
        });
    Ok(polygons.fold(MultiPolygon::new(vec![]), |all, p| all.union(&p)))
}

/// `<cache_prefix>/<hash of key>.cache`.
fn cache_path(cache_prefix: Option<String>, key: &str) -> String {
    let prefix = cache_prefix.unwrap_or("./".into());
    format!(
        "{prefix}/{}.cache",
        hex::encode(Sha256::digest(key.as_bytes()))
    )
    .replace("//", "/")
}

/// The value cached at `path`, else built and cached.
async fn cached<T: Serialize + DeserializeOwned>(
    path: &str,
    force_rebuild: bool,
    build: impl Future<Output = RResult<T>>,
) -> RResult<T> {
    if !force_rebuild
        && let Ok(bytes) = fs::read(path)
        && let Ok(value) = postcard::from_bytes(&bytes)
    {
        return Ok(value);
    }
    let value = build.await?;
    fs::write(path, postcard::to_stdvec(&value)?)?;
    Ok(value)
}

/// A URL downloaded once into the temporary directory, or a local path.
async fn download(url_or_path: &str, extension: &str) -> RResult<PathBuf> {
    if url::Url::parse(url_or_path).is_err() {
        return Ok(url_or_path.into());
    }
    let path = env::temp_dir().join(format!(
        "{}{extension}",
        hex::encode(Sha256::digest(url_or_path.as_bytes()))
    ));
    if !path.exists() {
        log::info!("Downloading {url_or_path} to {}...", path.display());
        let bytes = reqwest::get(url_or_path).await?.bytes().await?;
        std::io::copy(&mut Cursor::new(bytes), &mut fs::File::create(&path)?)?;
    }
    Ok(path)
}

#[cfg(feature = "hectare")]
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
pub struct HectareRecord {
    pub reli: u64,
    pub longitude: f64,
    pub latitude: f64,
    pub population: u64,
    pub area: Option<f64>,
}

/// The population per hectare (STATPOP), from the URL or path of its ZIP archive.
#[cfg(feature = "hectare")]
pub async fn hectare_records(
    url_or_path: &str,
    force_rebuild: bool,
    cache_prefix: Option<String>,
) -> RResult<Vec<HectareRecord>> {
    let build = async {
        let archive = download(url_or_path, ".zip").await?;
        let directory = archive.with_extension("");
        if !directory.exists() {
            log::info!(
                "Unzipping {} into {}...",
                archive.display(),
                directory.display()
            );
            zip::ZipArchive::new(fs::File::open(&archive)?)?.extract(&directory)?;
        }
        let csv = directory.join("ag-b-00.03-vz2023statpop/STATPOP2023.csv");
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_path(csv)?;
        reader
            .records()
            .map(|record| {
                let record = record?;
                let (latitude, longitude) =
                    super::utils::lv95_to_wgs84(record[3].parse()?, record[4].parse()?);
                let (reli, population) = (record[2].parse()?, record[5].parse()?);
                Ok(HectareRecord {
                    reli,
                    longitude,
                    latitude,
                    population,
                    area: None,
                })
            })
            .collect()
    };
    cached(&cache_path(cache_prefix, url_or_path), force_rebuild, build).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{LineString, Polygon};

    #[tokio::test]
    async fn cached_values_are_built_once() {
        let square = LineString::from(vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ]);
        let polygons = MultiPolygon::new(vec![Polygon::new(square, vec![])]);
        let path = env::temp_dir().join("test_cached_values_are_built_once.cache");
        let path = path.to_str().unwrap();
        let _ = fs::remove_file(path);

        let built = cached(path, false, async { Ok(polygons.clone()) })
            .await
            .unwrap();
        let loaded: MultiPolygon = cached(path, false, async { panic!("built twice") })
            .await
            .unwrap();
        assert_eq!((built, loaded), (polygons.clone(), polygons));
        fs::remove_file(path).unwrap();
    }
}
