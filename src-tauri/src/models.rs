use serde::Serialize;

/// A playable track, independent of where it comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    /// Id of the `MusicSource` that can stream this track.
    pub source: String,
    pub title: String,
    pub artist: String,
    pub duration_secs: u32,
    pub artwork_url: Option<String>,
    pub genre: Option<String>,
}
