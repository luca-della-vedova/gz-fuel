use futures_lite::future;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct FuelClient {
    pub url: String,
    pub cache_path: PathBuf,
    pub models: Vec<FuelModel>,
}

impl Default for FuelClient {
    fn default() -> Self {
        Self {
            url: "https://fuel.gazebosim.org/1.0/".into(),
            cache_path: Self::default_cache_path(),
            models: Default::default(),
        }
    }
}

impl FuelClient {
    async fn build_cache(&self) -> Option<Vec<FuelModel>> {
        println!("Building cache");
        let mut page = 1;
        let mut models = Vec::new();
        let models = loop {
            let url = self.url.clone() + "models" + "?page=" + &page.to_string();
            println!("Requesting page {}", page);
            let Ok(res) = surf::get(url.clone())
                .recv_string()
                .await else {
                break models;
            };
            let Ok(mut fetched_models) = serde_json::de::from_str::<Vec<FuelModel>>(&res) else {
                break models;
            };
            models.append(&mut fetched_models);
            dbg!(&models);
            page += 1;
        };
        if !models.is_empty() {
            Some(models)
        } else {
            None
        }
    }

    fn default_cache_path() -> PathBuf {
        let mut p = dirs::cache_dir().unwrap();
        p.push("open-robotics");
        p.push("gz-fuel");
        p.push("model_cache.json");
        p
    }

    fn last_updated(&self) -> Option<SystemTime> {
        let path = self.cache_path.clone();
        let cache_file = std::fs::File::open(path).ok()?;
        let metadata = cache_file.metadata().ok()?;
        metadata.modified().ok()
    }

    /// If threshold is None, only update if cache is not found
    fn should_update_cache(&self, threshold: &Option<Duration>) -> bool {
        let Some(last_updated) = self.last_updated() else {
            return true;
        };
        match threshold {
            Some(d) => SystemTime::now()
                .duration_since(last_updated)
                .is_ok_and(|dt| dt > *d),
            None => false,
        }
    }

    /// Returns Some if cache writing was successful, None otherwise
    async fn update_cache(&mut self) -> Option<()> {
        self.models = self.build_cache().await?;
        let path = self.cache_path.clone();
        fs::create_dir_all(path.parent()?).ok()?;
        let bytes = serde_json::ser::to_string_pretty(&self.models).ok()?;
        fs::write(path, bytes).ok()?;
        Some(())
    }

    fn update_cache_blocking(&mut self) -> Option<()> {
        future::block_on(self.update_cache())
    }
}

// TODO(luca) decide which fields we should skip to save on memory footprint
#[derive(Serialize, Deserialize, Debug)]
pub struct FuelModel {
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub name: String,
    pub owner: String,
    pub description: String,
    pub likes: u32,
    pub downloads: u32,
    pub filesize: usize,
    pub upload_date: String,
    pub modify_date: String,
    pub license_id: u32,
    pub license_name: String,
    pub license_url: String,
    pub license_image: String,
    pub permission: u32,
    pub url_name: String,
    pub private: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

fn main() {
    let mut client = FuelClient::default();
    dbg!(&client.cache_path);
    let last_updated = client.last_updated();
    dbg!(&last_updated);
    let should_update = client.should_update_cache(&Some(Duration::from_secs(100000)));
    dbg!(&should_update);
    if should_update {
        client.update_cache_blocking();
    }
}
