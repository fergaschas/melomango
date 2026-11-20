//! The commands the frontend calls (`invoke`), grouped by area.

use serde::Serialize;

pub(crate) use crate::*;

pub(crate) mod app;
pub(crate) mod downloads;
pub(crate) mod library;
pub(crate) mod pages;
pub(crate) mod plugins;
pub(crate) mod services;

/// What a failed command answers: its message, as a plain string for the frontend (as
/// before). Any `anyhow` error, `String` or `&str` converts with `?`, with its causes.
#[derive(Debug)]
pub(crate) struct CmdError(String);

pub(crate) type CmdResult<T> = Result<T, CmdError>;

impl Serialize for CmdError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl From<anyhow::Error> for CmdError {
    fn from(e: anyhow::Error) -> Self {
        Self(format!("{e:#}"))
    }
}

impl From<String> for CmdError {
    fn from(e: String) -> Self {
        Self(e)
    }
}

impl From<&str> for CmdError {
    fn from(e: &str) -> Self {
        Self(e.to_string())
    }
}

/// Runs `f` with the app state on the pool meant for blocking work (SQLite, files).
///
/// A Tauri command without `async` runs on the window's thread: a slow one (importing a
/// Spotify export, emptying the cache) froze the window until Windows said «No responde»,
/// and even a quick one waited there whenever another command held the database. Async
/// commands must not do this work inline either, or they stall the runtime's threads.
pub(crate) async fn blocking<T, F>(app: tauri::AppHandle, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> CmdResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| format!("la tarea se interrumpió: {e}"))?
}

/// `blocking` for library work on owned data, from async commands.
pub(crate) async fn with_library<T, F>(app: &tauri::AppHandle, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Library) -> anyhow::Result<T> + Send + 'static,
{
    blocking(app.clone(), move |s| f(&s.library).map_err(CmdError::from)).await
}

/// Stores tracks shown on catalog pages so they can be played (matched to YouTube) later.
pub(crate) async fn remember(app: &tauri::AppHandle, tracks: &[Track]) -> CmdResult<()> {
    let tracks = tracks.to_vec();
    with_library(app, move |l| l.remember_tracks(&tracks)).await
}
