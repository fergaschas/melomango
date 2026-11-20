//! Downloads, the automatic cache, and resolving streams ahead of time.

use super::*;

pub(crate) const DOWNLOAD_EVENT: &str = "download-progress";

/// Downloads songs in the background; progress arrives as `download-progress` events.
#[tauri::command]
#[specta::specta]
pub(crate) fn download_tracks(app: tauri::AppHandle, tracks: Vec<Track>) {
    for track in tracks {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let registry = state.registry();
            state
                .downloads
                .download(&registry, &state.library, &state.http, &track, StoredKind::Download, |progress| {
                    let _ = app.emit(DOWNLOAD_EVENT, progress);
                })
                .await;
        });
    }
}

/// Cancels a download in progress or deletes a downloaded song.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_download(app: tauri::AppHandle, source: String, id: String) -> CmdResult<()> {
    blocking(app, move |state| state.downloads.remove(&state.library, &source, &id).map_err(CmdError::from)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_downloads(app: tauri::AppHandle) -> CmdResult<Vec<Download>> {
    with_library(&app, |l| l.downloads()).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn downloaded_keys(app: tauri::AppHandle) -> CmdResult<Vec<String>> {
    with_library(&app, |l| l.downloaded_keys()).await
}

/// Bytes the automatic cache may use (0 turns it off and empties it).
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_cache_limit(app: tauri::AppHandle, bytes: u64) -> CmdResult<()> {
    blocking(app, move |state| state.downloads.set_cache_limit(&state.library, bytes).map_err(CmdError::from)).await
}

#[derive(Serialize, specta::Type)]
pub(crate) struct CacheUsage {
    songs: u32,
    bytes: u64,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cache_usage(app: tauri::AppHandle) -> CmdResult<CacheUsage> {
    let (songs, bytes) = with_library(&app, |l| l.cache_usage()).await?;
    Ok(CacheUsage { songs, bytes })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_cache(app: tauri::AppHandle) -> CmdResult<()> {
    blocking(app, |state| state.downloads.clear_cache(&state.library).map_err(CmdError::from)).await
}

/// Resolves a track's audio in the background so it starts at once when played.
#[tauri::command]
#[specta::specta]
pub(crate) fn prefetch_stream(app: tauri::AppHandle, source: String, id: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let registry = state.registry();
        // Best effort: a failure here is retried (and reported) when the track is played.
        let _ = stream::prefetch(&registry, &state.library, &state.http, &state.streams, &source, &id).await;
    });
}
