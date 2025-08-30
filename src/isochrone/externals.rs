use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

use bincode::config;
use geo::{BooleanOps, MultiPolygon, Polygon};
use geojson::{FeatureCollection, GeoJson};
use sha2::{Digest, Sha256};
use url::Url;

pub const LAKES_GEOJSON_URLS: [&str; 20] = [
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-baldegg.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-biel.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-brienz.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-constance.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-geneva.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-hallwil.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-lac-de-joux.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-lucerne.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-lugano.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-maggiore.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-morat.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-neuchatel.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-of-gruyere.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-sarnen.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-sempach.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-sihl.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-thun.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-wagitalersee.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-walensee.geojson",
    "https://raw.githubusercontent.com/ZHB/switzerland-geojson/05cc91014860ddd8a6c1704f4a421f1e9b1f0080/lakes/lake-zurich.geojson",
];

fn parse_geojson_file(path: &str) -> Result<MultiPolygon, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Parse the GeoJSON file
    let geojson: GeoJson = serde_json::from_reader(reader)?;

    let polygons = FeatureCollection::try_from(geojson)?
        .into_iter()
        .filter_map(|feature| {
            feature.geometry.and_then(|geometry| {
                if let geojson::Value::Polygon(exteriors) = geometry.value {
                    let polygons: MultiPolygon = exteriors
                        .into_iter()
                        .map(|exterior| {
                            Polygon::new(
                                exterior
                                    .into_iter()
                                    // The coordinates are inverted. It's normal
                                    .map(|coords| (coords[1], coords[0]))
                                    .collect(),
                                vec![],
                            )
                        })
                        .collect();
                    Some(polygons)
                } else {
                    None
                }
            })
        })
        .fold(MultiPolygon::new(vec![]), |res, p| res.union(&p));
    Ok(polygons)
}

pub struct ExcludedPolygons;

impl ExcludedPolygons {
    fn build_cache(multis: &MultiPolygon, path: &str) -> Result<(), Box<dyn Error>> {
        let data = bincode::serde::encode_to_vec(multis, config::standard())?;
        std::fs::write(path, data)?;
        Ok(())
    }

    fn load_from_cache(path: &str) -> Result<MultiPolygon, Box<dyn Error>> {
        let data = std::fs::read(path)?;
        let (multis, _) = bincode::serde::decode_from_slice(&data, config::standard())?;
        Ok(multis)
    }

    pub async fn try_new(
        urls: &[&str],
        force_rebuild_cache: bool,
        cache_prefix: Option<String>,
    ) -> Result<MultiPolygon, Box<dyn Error>> {
        let cache_path = format!(
            "{}/{:x}.cache",
            cache_prefix.unwrap_or("./".to_string()),
            Sha256::digest(
                urls.iter()
                    .fold(String::new(), |res, &s| res + s)
                    .as_bytes(),
            )
        )
        .replace("//", "/");

        let multis = if !force_rebuild_cache && Path::new(&cache_path).exists() {
            Self::load_from_cache(&cache_path)?
        } else {
            let mut multis = Vec::new();
            for &url in urls {
                let unique_filename = format!("{:x}", Sha256::digest(url.as_bytes()));

                // The cache must be built.
                // If cache loading has failed, the cache must be rebuilt.
                let data_path = if Url::parse(url).is_ok() {
                    let data_path = format!("/tmp/{unique_filename}");

                    if !Path::new(&data_path).exists() {
                        // The data must be downloaded.
                        log::info!("Downloading GeoJson data to {data_path}...");
                        let response = reqwest::get(url).await?;
                        let mut file = std::fs::File::create(&data_path)?;
                        let mut content = Cursor::new(response.bytes().await?);
                        std::io::copy(&mut content, &mut file)?;
                    }

                    data_path
                } else {
                    url.to_string()
                };

                log::info!("Parsing ExcludedPolygons data from {data_path}...");
                let local = parse_geojson_file(&data_path)?;

                multis.push(local);
            }

            let multis = multis
                .into_iter()
                .fold(MultiPolygon::new(vec![]), |poly, p| poly.union(&p));
            Self::build_cache(&multis, &cache_path)?;
            multis
        };

        Ok(multis)
    }
}
