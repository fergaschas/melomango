pub mod plugin;

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use crate::models::{SourceInfo, StreamInfo, Track};

/// A place music comes from. Each source provides both its catalogue and the audio.
///
/// Sources implement this; today all of them are installable plugins, which are
/// external programs wrapped by `plugin::PluginSource` (protocol in `plugins/PROTOCOL.md`).
#[async_trait]
pub trait MusicSource: Send + Sync {
    fn info(&self) -> SourceInfo;
    async fn search(&self, query: &str) -> anyhow::Result<Vec<Track>>;
    async fn trending(&self) -> anyhow::Result<Vec<Track>>;
    /// Where to get the track's audio. The URL may redirect and must accept HTTP Range requests.
    async fn stream(&self, track_id: &str) -> anyhow::Result<StreamInfo>;
    /// Songs like `track_id`, for the automatic radio; `None` if the source can't tell.
    async fn radio(&self, _track_id: &str) -> anyhow::Result<Option<Vec<Track>>> {
        Ok(None)
    }
}

/// The sources currently available, in display order.
#[derive(Default)]
pub struct Registry {
    sources: Vec<Arc<dyn MusicSource>>,
}

impl Registry {
    pub fn new(sources: Vec<Arc<dyn MusicSource>>) -> Self {
        Self { sources }
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Arc<dyn MusicSource>> {
        self.sources.iter().find(|s| s.info().id == id).cloned().ok_or_else(|| anyhow!("fuente desconocida: {id}"))
    }

    pub fn list(&self) -> Vec<SourceInfo> {
        self.sources.iter().map(|s| s.info()).collect()
    }
}
