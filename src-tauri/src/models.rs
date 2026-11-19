use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A playable track, independent of where it comes from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    /// Id of the `MusicSource` that can stream this track. Plugins may omit it:
    /// the host fills it in.
    #[serde(default)]
    pub source: String,
    pub title: String,
    pub artist: String,
    pub duration_secs: u32,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
}

/// Where to fetch a track's audio from, plus any headers the server expects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Builtin,
    Plugin,
}

/// What the UI needs to know about a source.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    pub version: Option<String>,
    pub description: Option<String>,
    pub supports_trending: bool,
}
