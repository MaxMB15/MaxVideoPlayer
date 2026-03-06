use crate::models::channel::Channel;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XtreamError {
    #[error("authentication failed")]
    AuthFailed,
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone)]
pub struct XtreamCredentials {
    pub server: String,
    pub username: String,
    pub password: String,
}

impl XtreamCredentials {
    pub fn new(server: &str, username: &str, password: &str) -> Self {
        let server = server.trim_end_matches('/').to_string();
        Self {
            server,
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    fn api_url(&self, action: &str) -> String {
        format!(
            "{}/player_api.php?username={}&password={}&action={action}",
            self.server, self.username, self.password
        )
    }

    fn stream_url(&self, stream_id: u64, extension: &str) -> String {
        format!(
            "{}/live/{}/{}/{stream_id}.{extension}",
            self.server, self.username, self.password
        )
    }
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    user_info: Option<UserInfo>,
    #[allow(dead_code)]
    server_info: Option<ServerInfo>,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    auth: Option<u8>,
    status: Option<String>,
    #[allow(dead_code)]
    username: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ServerInfo {
    #[allow(dead_code)]
    url: Option<String>,
    #[allow(dead_code)]
    port: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XtreamCategory {
    category_id: String,
    category_name: String,
}

#[derive(Debug, Deserialize)]
struct XtreamStream {
    #[allow(dead_code)]
    num: Option<u64>,
    name: Option<String>,
    stream_id: Option<u64>,
    stream_icon: Option<String>,
    epg_channel_id: Option<String>,
    category_id: Option<String>,
    #[serde(default)]
    container_extension: Option<String>,
}

pub struct XtreamClient {
    credentials: XtreamCredentials,
    client: reqwest::Client,
}

impl XtreamClient {
    pub fn new(credentials: XtreamCredentials) -> Self {
        Self {
            credentials,
            client: reqwest::Client::new(),
        }
    }

    pub async fn authenticate(&self) -> Result<(), XtreamError> {
        let url = self.credentials.api_url("");
        let resp: AuthResponse = self.client.get(&url).send().await?.json().await?;

        match resp.user_info {
            Some(info) => {
                let authed = info.auth == Some(1)
                    || info.status.as_deref() == Some("Active");
                if authed {
                    Ok(())
                } else {
                    Err(XtreamError::AuthFailed)
                }
            }
            None => Err(XtreamError::AuthFailed),
        }
    }

    pub async fn get_live_categories(&self) -> Result<Vec<(String, String)>, XtreamError> {
        let url = self.credentials.api_url("get_live_categories");
        let cats: Vec<XtreamCategory> = self.client.get(&url).send().await?.json().await?;
        Ok(cats
            .into_iter()
            .map(|c| (c.category_id, c.category_name))
            .collect())
    }

    pub async fn get_live_streams(&self) -> Result<Vec<Channel>, XtreamError> {
        let url = self.credentials.api_url("get_live_streams");
        let streams: Vec<XtreamStream> = self.client.get(&url).send().await?.json().await?;

        let categories = self.get_live_categories().await?;
        let cat_map: std::collections::HashMap<String, String> = categories.into_iter().collect();

        let channels: Vec<Channel> = streams
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let stream_id = s.stream_id?;
                let name = s.name.unwrap_or_else(|| format!("Stream {stream_id}"));
                let ext = s.container_extension.as_deref().unwrap_or("ts");
                let group = s
                    .category_id
                    .as_ref()
                    .and_then(|cid| cat_map.get(cid))
                    .cloned()
                    .unwrap_or_else(|| "Uncategorized".to_string());

                Some(Channel {
                    id: format!("xt-{i}"),
                    name,
                    url: self.credentials.stream_url(stream_id, ext),
                    logo_url: s.stream_icon.filter(|u| !u.is_empty()),
                    group_title: group,
                    tvg_id: s.epg_channel_id,
                    tvg_name: None,
                    is_favorite: false,
                })
            })
            .collect();

        Ok(channels)
    }

    pub async fn get_vod_streams(&self) -> Result<Vec<Channel>, XtreamError> {
        let url = self.credentials.api_url("get_vod_streams");
        let streams: Vec<XtreamStream> = self.client.get(&url).send().await?.json().await?;

        let channels: Vec<Channel> = streams
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let stream_id = s.stream_id?;
                let name = s.name.unwrap_or_else(|| format!("VOD {stream_id}"));
                let ext = s.container_extension.as_deref().unwrap_or("mp4");

                Some(Channel {
                    id: format!("vod-{i}"),
                    name,
                    url: format!(
                        "{}/movie/{}/{}/{stream_id}.{ext}",
                        self.credentials.server,
                        self.credentials.username,
                        self.credentials.password
                    ),
                    logo_url: s.stream_icon.filter(|u| !u.is_empty()),
                    group_title: "VOD".to_string(),
                    tvg_id: None,
                    tvg_name: None,
                    is_favorite: false,
                })
            })
            .collect();

        Ok(channels)
    }
}

/// Convenience function: authenticate and fetch all live streams.
pub async fn fetch_xtream_channels(
    server: &str,
    username: &str,
    password: &str,
) -> Result<Vec<Channel>, XtreamError> {
    let creds = XtreamCredentials::new(server, username, password);
    let client = XtreamClient::new(creds);
    client.authenticate().await?;
    client.get_live_streams().await
}
