use serde::{Deserialize, Serialize};

use super::channel::{Category, Channel};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub last_updated: Option<String>,
    pub channel_count: usize,
    pub epg_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    M3u,
    Xtream,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub provider: Provider,
    pub channels: Vec<Channel>,
    pub categories: Vec<Category>,
}

impl Playlist {
    pub fn new(provider: Provider, channels: Vec<Channel>) -> Self {
        let categories = Category::from_channels(&channels);
        Self {
            provider,
            channels,
            categories,
        }
    }
}
