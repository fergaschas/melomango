//! Audius: decentralised music platform with a free, open API and full-length streams.
//! Docs: https://docs.audius.org/developers/api

use anyhow::{bail, Context};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::OnceCell;

use super::MusicSource;
use crate::models::Track;

const APP_NAME: &str = "melomango";
const DISCOVERY_URL: &str = "https://api.audius.co";

pub struct Audius {
    http: reqwest::Client,
    host: OnceCell<String>,
}

impl Audius {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            host: OnceCell::new(),
        }
    }

    /// The discovery endpoint answers with the list of API hosts to use.
    async fn host(&self) -> anyhow::Result<&str> {
        let host = self
            .host
            .get_or_try_init(|| async {
                let hosts: ApiResponse<Vec<String>> = self
                    .http
                    .get(DISCOVERY_URL)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await
                    .context("respuesta inesperada del descubrimiento de Audius")?;
                Ok::<_, anyhow::Error>(
                    hosts
                        .data
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| DISCOVERY_URL.to_string()),
                )
            })
            .await?;
        Ok(host)
    }

    async fn tracks(&self, path: &str, params: &[(&str, &str)]) -> anyhow::Result<Vec<Track>> {
        let host = self.host().await?;
        let resp: ApiResponse<Vec<ApiTrack>> = self
            .http
            .get(format!("{host}/v1{path}"))
            .query(&[("app_name", APP_NAME)])
            .query(params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("respuesta inesperada de Audius")?;
        Ok(resp
            .data
            .into_iter()
            // Gated (premium / follow-to-unlock) tracks report is_streamable = false.
            .filter(|t| t.is_streamable != Some(false))
            .map(Track::from)
            .collect())
    }
}

#[async_trait]
impl MusicSource for Audius {
    fn id(&self) -> &'static str {
        "audius"
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Track>> {
        self.tracks("/tracks/search", &[("query", query)]).await
    }

    async fn trending(&self) -> anyhow::Result<Vec<Track>> {
        self.tracks("/tracks/trending", &[("limit", "50")]).await
    }

    async fn stream_url(&self, track_id: &str) -> anyhow::Result<String> {
        if track_id.is_empty() || !track_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("id de pista no válido: {track_id}");
        }
        let host = self.host().await?;
        Ok(format!("{host}/v1/tracks/{track_id}/stream?app_name={APP_NAME}"))
    }
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Deserialize)]
struct ApiTrack {
    id: String,
    title: String,
    duration: f64,
    genre: Option<String>,
    artwork: Option<ApiArtwork>,
    user: ApiUser,
    is_streamable: Option<bool>,
}

#[derive(Deserialize)]
struct ApiArtwork {
    #[serde(rename = "480x480")]
    medium: Option<String>,
    #[serde(rename = "150x150")]
    small: Option<String>,
}

#[derive(Deserialize)]
struct ApiUser {
    name: String,
}

impl From<ApiTrack> for Track {
    fn from(t: ApiTrack) -> Self {
        Track {
            id: t.id,
            source: "audius".into(),
            title: t.title,
            artist: t.user.name,
            duration_secs: t.duration.max(0.0).round() as u32,
            artwork_url: t.artwork.and_then(|a| a.medium.or(a.small)),
            genre: t.genre.filter(|g| !g.is_empty()),
        }
    }
}
