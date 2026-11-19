pub mod audius;

use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;

use crate::models::Track;

/// A place music comes from. Each source provides both its catalogue and the audio.
///
/// To add a new backend (Jamendo, Subsonic/Navidrome, Internet Archive...), implement
/// this trait and register it in `lib.rs`. A source that only provides audio for an
/// external catalogue would need a matcher in front of it.
#[async_trait]
pub trait MusicSource: Send + Sync {
    fn id(&self) -> &'static str;
    async fn search(&self, query: &str) -> anyhow::Result<Vec<Track>>;
    async fn trending(&self) -> anyhow::Result<Vec<Track>>;
    /// URL serving the track's audio. It may redirect and must accept HTTP Range requests.
    async fn stream_url(&self, track_id: &str) -> anyhow::Result<String>;
}

pub struct Registry {
    sources: HashMap<&'static str, Arc<dyn MusicSource>>,
}

impl Registry {
    pub fn new(sources: Vec<Arc<dyn MusicSource>>) -> Self {
        Self {
            sources: sources.into_iter().map(|s| (s.id(), s)).collect(),
        }
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Arc<dyn MusicSource>> {
        self.sources
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("fuente desconocida: {id}"))
    }
}
