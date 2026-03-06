use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub url: String,
    pub logo_url: Option<String>,
    pub group_title: String,
    pub tvg_id: Option<String>,
    pub tvg_name: Option<String>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub channel_count: usize,
}

impl Category {
    pub fn from_channels(channels: &[Channel]) -> Vec<Category> {
        let mut map = std::collections::HashMap::<String, usize>::new();
        for ch in channels {
            *map.entry(ch.group_title.clone()).or_default() += 1;
        }
        let mut cats: Vec<Category> = map
            .into_iter()
            .enumerate()
            .map(|(i, (name, count))| Category {
                id: format!("cat-{i}"),
                name,
                channel_count: count,
            })
            .collect();
        cats.sort_by(|a, b| a.name.cmp(&b.name));
        cats
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub stream_type: StreamType,
    pub container: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StreamType {
    Live,
    Vod,
    Series,
}
