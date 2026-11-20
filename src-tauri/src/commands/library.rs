//! The user's library: likes, playlists, history and which recording plays each song.

use super::*;

#[tauri::command]
#[specta::specta]
pub(crate) async fn liked_tracks(app: tauri::AppHandle) -> CmdResult<Vec<Track>> {
    with_library(&app, |l| l.liked_tracks()).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn liked_keys(app: tauri::AppHandle) -> CmdResult<Vec<String>> {
    with_library(&app, |l| l.liked_keys()).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_liked(app: tauri::AppHandle, track: Track, liked: bool) -> CmdResult<()> {
    with_library(&app, move |l| l.set_liked(&track, liked)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_playlists(app: tauri::AppHandle) -> CmdResult<Vec<PlaylistSummary>> {
    with_library(&app, |l| l.playlists()).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_playlist(app: tauri::AppHandle, name: String) -> CmdResult<PlaylistSummary> {
    with_library(&app, move |l| l.create_playlist(&name)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn rename_playlist(app: tauri::AppHandle, id: i64, name: String) -> CmdResult<()> {
    with_library(&app, move |l| l.rename_playlist(id, &name)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_playlist(app: tauri::AppHandle, id: i64) -> CmdResult<()> {
    with_library(&app, move |l| l.delete_playlist(id)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_playlist(app: tauri::AppHandle, id: i64) -> CmdResult<Playlist> {
    with_library(&app, move |l| l.playlist(id)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn add_to_playlist(app: tauri::AppHandle, id: i64, track: Track) -> CmdResult<bool> {
    with_library(&app, move |l| l.add_to_playlist(id, &track)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_from_playlist(
    app: tauri::AppHandle,
    id: i64,
    source: String,
    track_id: String,
) -> CmdResult<()> {
    with_library(&app, move |l| l.remove_from_playlist(id, &source, &track_id)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn move_in_playlist(app: tauri::AppHandle, id: i64, from: usize, to: usize) -> CmdResult<()> {
    with_library(&app, move |l| l.move_in_playlist(id, from, to)).await
}

/// Songs like `track` for the automatic radio, leaving out `exclude` ("source/id" already queued).
#[tauri::command]
#[specta::specta]
pub(crate) async fn radio_tracks(
    state: State<'_, AppState>,
    track: Track,
    exclude: Vec<String>,
) -> CmdResult<Vec<Track>> {
    let registry = state.registry();
    let exclude = exclude.into_iter().collect();
    radio::tracks(&registry, &state.library, &state.http, &track, &exclude).await.map_err(CmdError::from)
}

/// Records that the user listened to `track` (called once it has played a while).
#[tauri::command]
#[specta::specta]
pub(crate) async fn record_play(app: tauri::AppHandle, track: Track) -> CmdResult<()> {
    let played = track.clone();
    with_library(&app, move |l| l.record_play(&played)).await?;
    // Played often (or liked): keep a copy in the cache, in the background.
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let registry = state.registry();
        state.downloads.cache_if_favourite(&registry, &state.library, &state.http, &track).await;
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn recent_plays(app: tauri::AppHandle, limit: Option<u32>) -> CmdResult<Vec<Play>> {
    with_library(&app, move |l| l.recent_plays(limit.unwrap_or(500).min(5_000))).await
}

/// Months with plays ("YYYY-MM"), newest first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn summary_months(app: tauri::AppHandle) -> CmdResult<Vec<String>> {
    with_library(&app, |l| l.summary_months()).await
}

/// "Tu resumen del mes": totals and favourites of `month` ("YYYY-MM").
#[tauri::command]
#[specta::specta]
pub(crate) async fn month_summary(app: tauri::AppHandle, month: String) -> CmdResult<MonthSummary> {
    with_library(&app, move |l| l.month_summary(&month)).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn clear_history(app: tauri::AppHandle) -> CmdResult<()> {
    with_library(&app, |l| l.clear_history()).await
}

/// Recordings to choose from when an imported track plays the wrong one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn match_options(
    state: State<'_, AppState>,
    source: String,
    id: String,
    query: Option<String>,
) -> CmdResult<matcher::MatchOptions> {
    let registry = state.registry();
    matcher::options(&registry, &state.library, &source, &id, query).await.map_err(CmdError::from)
}

/// Plays `source/id` from the recording the user chose from now on.
#[tauri::command]
#[specta::specta]
pub(crate) async fn choose_match(
    app: tauri::AppHandle,
    source: String,
    id: String,
    play_source: String,
    play_id: String,
) -> CmdResult<()> {
    blocking(app, move |state| {
        state.source(&play_source)?;
        state.library.set_track_match(&source, &id, &play_source, &play_id)?;
        state.streams.forget(&format!("{source}/{id}"));
        // A download of the old recording would keep playing it.
        state.downloads.remove(&state.library, &source, &id).map_err(CmdError::from)
    })
    .await
}
