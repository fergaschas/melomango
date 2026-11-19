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

/// "Song (feat. Someone)" -> "Song". Lyrics databases usually list the bare title.
pub fn trim_featuring(title: &str) -> &str {
    let lower = title.to_ascii_lowercase();
    ["(feat", "(ft.", "[feat", "[ft.", " feat. ", " ft. "]
        .iter()
        .filter_map(|marker| lower.find(marker))
        .min()
        .map_or(title, |cut| title[..cut].trim_end())
}

#[cfg(test)]
mod tests {
    use super::trim_featuring;

    #[test]
    fn trims_featuring() {
        assert_eq!(trim_featuring("MIA (feat. Drake)"), "MIA");
        assert_eq!(trim_featuring("Song [Ft. X]"), "Song");
        assert_eq!(trim_featuring("Song feat. X"), "Song");
        assert_eq!(trim_featuring("Feather"), "Feather");
    }
}
