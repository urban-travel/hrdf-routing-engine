use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufReader, Cursor};
use std::path::Path;

use geo::{BooleanOps, MultiPolygon, Polygon};
use geojson::{FeatureCollection, GeoJson};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

#[cfg(feature = "hectare")]
use zip::ZipArchive;

use super::utils::lv95_to_wgs84;

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
        let bytes = postcard::to_stdvec(multis)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    fn load_from_cache(path: &str) -> Result<MultiPolygon, Box<dyn Error>> {
        let data = std::fs::read(path)?;
        let multis: MultiPolygon = postcard::from_bytes(&data)?;
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

#[cfg(feature = "hectare")]
#[derive(Debug, Serialize, Deserialize)]
pub struct HectareData {
    data: Vec<HectareRecord>,
}

#[cfg(feature = "hectare")]
impl PartialEq for HectareData {
    fn eq(&self, other: &Self) -> bool {
        self.data
            .iter()
            .zip(other.data.iter())
            .all(|(lhs, rhs)| lhs.eq(rhs))
    }
}

#[cfg(feature = "hectare")]
impl HectareData {
    /// Loads and parses the data.
    /// If an URL is provided, the data containing the population per hectare is loaded from the specified URL which is downloaded automatically.
    /// If a path is provided, it must absolutely point to an valid archive (ZIP file).
    /// The ZIP archive is automatically decompressed into the temp_dir of the OS folder.
    pub async fn new(
        url_or_path: &str,
        force_rebuild_cache: bool,
        cache_prefix: Option<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let unique_filename = format!("{:x}", Sha256::digest(url_or_path.as_bytes()));
        let cache_path = format!(
            "{}/{unique_filename}.cache",
            cache_prefix.unwrap_or(String::from("./"))
        )
        .replace("//", "/");

        let hectare = if Path::new(&cache_path).exists() && !force_rebuild_cache {
            // Loading from cache.
            log::info!("Loading Hectare data from cache ({cache_path})...");

            // If loading from cache fails, None is returned.
            HectareData::load_from_cache(&cache_path).ok()
        } else {
            // No loading from cache.
            None
        };

        let hectare = if let Some(hectare) = hectare {
            // The cache has been loaded without error.
            hectare
        } else {
            // The cache must be built.
            // If cache loading has failed, the cache must be rebuilt.
            let compressed_data_path = if Url::parse(url_or_path).is_ok() {
                let compressed_data_path = env::temp_dir()
                    .join(format!("{unique_filename}.zip"))
                    .into_os_string()
                    .into_string()
                    .expect("Could not convert to string.");

                if !Path::new(&compressed_data_path).exists() {
                    // The data must be downloaded.
                    log::info!("Downloading HECTARE data to {compressed_data_path}...");
                    let response = reqwest::get(url_or_path).await?;
                    let mut file = std::fs::File::create(&compressed_data_path)?;
                    let mut content = Cursor::new(response.bytes().await?);
                    std::io::copy(&mut content, &mut file)?;
                }

                compressed_data_path
            } else {
                url_or_path.to_string()
            };

            let decompressed_data_path = env::temp_dir()
                .join(unique_filename)
                .into_os_string()
                .into_string()
                .expect("Could not convert to string.");

            if !Path::new(&decompressed_data_path).exists() {
                // The data must be decompressed.
                log::info!("Unzipping HECTARE archive into {decompressed_data_path}...");
                let file = File::open(&compressed_data_path)?;
                let mut archive = ZipArchive::new(BufReader::new(file))?;
                archive.extract(&decompressed_data_path)?;
            }

            log::info!("Parsing HECTARE data from {decompressed_data_path}...");

            let hectare = Self {
                data: Self::parse(&decompressed_data_path)?,
            };

            log::info!("Building cache...");
            hectare.build_cache(&cache_path)?;
            hectare
        };

        Ok(hectare)
    }

    fn parse(decompressed_data_path: &str) -> Result<Vec<HectareRecord>, Box<dyn Error>> {
        let path = format!("{decompressed_data_path}/ag-b-00.03-vz2023statpop/STATPOP2023.csv");
        let file = File::open(path)?;

        let mut rdr = csv::ReaderBuilder::new().delimiter(b';').from_reader(file);
        rdr.records()
            .map(|result| {
                let record = result?;

                let reli: u64 = record[2].parse()?;
                let easting: f64 = record[3].parse()?;
                let northing: f64 = record[4].parse()?;
                let population: u64 = record[5].parse()?;

                let (latitude, longitude) = lv95_to_wgs84(easting, northing);
                // println!("{latitude}, {longitude}");
                Ok(HectareRecord {
                    reli,
                    longitude,
                    latitude,
                    population,
                    area: None,
                })
            })
            .collect()
    }

    pub fn data(self) -> Vec<HectareRecord> {
        self.data
    }

    fn build_cache(&self, path: &str) -> Result<(), Box<dyn Error>> {
        let bytes = postcard::to_stdvec(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    fn load_from_cache(path: &str) -> Result<Self, Box<dyn Error>> {
        let data = fs::read(path)?;
        let hectare: HectareData = postcard::from_bytes(&data)?;
        Ok(hectare)
    }
}

#[cfg(feature = "hectare")]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HectareRecord {
    pub reli: u64,
    pub longitude: f64,
    pub latitude: f64,
    pub population: u64,
    pub area: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{LineString, Polygon};
    use std::fs;

    #[test]
    fn test_excluded_polygons_cache() {
        // Create a test MultiPolygon
        let exterior = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ];
        let polygon = Polygon::new(LineString::from(exterior), vec![]);
        let multi_polygon = MultiPolygon::new(vec![polygon]);

        let cache_path = "/tmp/test_excluded_polygons.cache";
        let _ = fs::remove_file(cache_path);
        ExcludedPolygons::build_cache(&multi_polygon, cache_path)
            .unwrap_or_else(|_| panic!("Failed to build cache {cache_path}"));

        assert!(
            std::path::Path::new(cache_path).exists(),
            "Cache file should exist"
        );

        let loaded_polygon = ExcludedPolygons::load_from_cache(cache_path)
            .unwrap_or_else(|_| panic!("Failed to load from {cache_path}"));

        assert_eq!(
            multi_polygon.0.len(),
            loaded_polygon.0.len(),
            "Number of polygons should match"
        );

        fs::remove_file(cache_path)
            .unwrap_or_else(|_| panic!("Failed to remove cache file {cache_path}"));
    }

    #[test]
    #[cfg(feature = "hectare")]
    fn test_hectare_data_cache() {
        let test_records = vec![
            HectareRecord {
                reli: 1,
                longitude: 6.14,
                latitude: 46.21,
                population: 1000,
                area: Some(100.0),
            },
            HectareRecord {
                reli: 2,
                longitude: 7.44,
                latitude: 46.95,
                population: 2000,
                area: Some(150.0),
            },
        ];

        let hectare_data = HectareData {
            data: test_records.clone(),
        };
        let cache_path = "/tmp/test_hectare.cache";
        let _ = fs::remove_file(cache_path);
        hectare_data
            .build_cache(cache_path)
            .unwrap_or_else(|_| panic!("Failed to build cache {cache_path}"));

        assert!(
            std::path::Path::new(cache_path).exists(),
            "Cache file should exist"
        );

        let loaded_data = HectareData::load_from_cache(cache_path)
            .unwrap_or_else(|_| panic!("Failed to load from {cache_path}"));
        assert_eq!(
            hectare_data.data.len(),
            loaded_data.data.len(),
            "Number of records should match"
        );

        for (original, loaded) in hectare_data.data.iter().zip(loaded_data.data.iter()) {
            assert_eq!(original.reli, loaded.reli);
            assert_eq!(original.longitude, loaded.longitude);
            assert_eq!(original.latitude, loaded.latitude);
            assert_eq!(original.population, loaded.population);
            assert_eq!(original.area, loaded.area);
        }
        fs::remove_file(cache_path)
            .unwrap_or_else(|_| panic!("Failed to remove cache file {cache_path}"));
    }
}
