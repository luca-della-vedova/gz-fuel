use futures_lite::future;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

// TODO(luca) clone can be unsafe if two instances try to write to the same file
#[derive(Clone)]
pub struct FuelClient {
    pub url: String,
    pub cache_path: PathBuf,
    pub models: Option<Vec<FuelModel>>,
    pub token: Option<String>,
}

impl Default for FuelClient {
    fn default() -> Self {
        let cache_path = Self::default_cache_path();
        Self {
            url: "https://fuel.gazebosim.org/1.0/".into(),
            models: fs::read(&cache_path)
                .ok()
                .and_then(|b| serde_json::de::from_slice::<Vec<FuelModel>>(&b).ok()),
            cache_path,
            token: None,
        }
    }
}

impl FuelClient {
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    async fn build_cache(&self) -> Option<Vec<FuelModel>> {
        let mut page = 1;
        let mut models = Vec::new();
        let models = loop {
            let url = self.url.clone() + "models" + "?page=" + &page.to_string();
            let mut req = surf::get(url.clone());
            if let Some(token) = &self.token {
                req = req.header("Private-token", token.clone());
            }
            let Ok(res) = req
                .recv_string()
                .await else {
                break models;
            };
            let Ok(mut fetched_models) = serde_json::de::from_str::<Vec<FuelModel>>(&res) else {
                break models;
            };
            models.append(&mut fetched_models);
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

    /// If threshold is None, only update if cache is not found, otherwise update if cache is older
    /// than threshold Duration
    pub fn should_update_cache(&self, threshold: &Option<Duration>) -> bool {
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
    pub async fn update_cache(&mut self) -> Option<()> {
        if let Some(models) = self.build_cache().await {
            self.models = Some(models);
            let path = self.cache_path.clone();
            fs::create_dir_all(path.parent()?).ok()?;
            let bytes = serde_json::ser::to_string_pretty(&self.models).ok()?;
            fs::write(path, bytes).ok()?;
            Some(())
        } else {
            None
        }
    }

    pub fn update_cache_blocking(&mut self) -> Option<()> {
        future::block_on(self.update_cache())
    }

    // Filtering functions, return cache filtered based on criteria
    pub fn models_by_owner(
        &self,
        models: Option<&Vec<FuelModel>>,
        owner: &str,
    ) -> Option<Vec<FuelModel>> {
        let models = models.or(self.models.as_ref())?;
        Some(
            models
                .iter()
                .filter(|model| model.owner == owner)
                .cloned()
                .collect::<Vec<_>>(),
        )
    }

    pub fn get_owners(&self) -> Option<Vec<String>> {
        let models = self.models.as_ref()?;
        Some(
            models
                .iter()
                .unique_by(|model| &model.owner)
                .clone()
                .map(|model| model.owner.clone())
                .sorted_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()))
                .collect::<Vec<_>>(),
        )
    }

    pub fn models_by_private(
        &self,
        models: Option<&Vec<FuelModel>>,
        private: bool,
    ) -> Option<Vec<FuelModel>> {
        let models = models.or(self.models.as_ref())?;
        Some(
            models
                .iter()
                .filter(|model| model.private == private)
                .cloned()
                .collect::<Vec<_>>(),
        )
    }

    pub fn get_tags(&self) -> Option<Vec<String>> {
        let models = self.models.as_ref()?;
        Some(
            models
                .iter()
                .flat_map(|model| &model.tags)
                .unique()
                .cloned()
                .sorted_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()))
                .collect::<Vec<_>>(),
        )
    }

    pub fn models_by_tag(
        &self,
        models: Option<&Vec<FuelModel>>,
        tag: &str,
    ) -> Option<Vec<FuelModel>> {
        let models = models.or(self.models.as_ref())?;
        Some(
            models
                .iter()
                .filter(|model| model.tags.contains(&tag.to_owned()))
                .cloned()
                .collect::<Vec<_>>(),
        )
    }
}

// TODO(luca) decide which fields we should skip to save on memory footprint
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
