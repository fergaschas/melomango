//! Catalogue pages from Deezer (artists, albums, public playlists) and lyrics.

use super::*;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_lyrics(state: State<'_, AppState>, track: Track) -> CmdResult<Lyrics> {
    state.lyrics.get(&track).await.map_err(CmdError::from)
}

/// An artist's page (popular songs, releases, similar artists), by Deezer id or by name.
#[tauri::command]
#[specta::specta]
pub(crate) async fn artist_page(app: tauri::AppHandle, name: String, id: Option<String>) -> CmdResult<ArtistPage> {
    let page = app.state::<AppState>().artists.artist(&name, id.as_deref()).await?;
    remember(&app, &page.top).await?;
    Ok(page)
}

/// Artists, albums and public playlists matching `query` (from Deezer).
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_search(state: State<'_, AppState>, query: String) -> CmdResult<CatalogSearch> {
    state.artists.search(&query).await.map_err(CmdError::from)
}

/// Artists and songs to suggest while typing a search (from Deezer).
#[tauri::command]
#[specta::specta]
pub(crate) async fn search_suggestions(app: tauri::AppHandle, query: String) -> CmdResult<Suggestions> {
    if query.trim().chars().count() < 2 {
        return Ok(Suggestions { artists: Vec::new(), tracks: Vec::new() });
    }
    let found = app.state::<AppState>().artists.suggest(&query).await?;
    // Suggested songs can be played straight away (matched to YouTube).
    remember(&app, &found.tracks).await?;
    Ok(found)
}

/// A public Deezer playlist with its songs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn deezer_playlist(app: tauri::AppHandle, id: String) -> CmdResult<PlaylistPage> {
    let page = app.state::<AppState>().artists.playlist(&id, false).await?;
    remember(&app, &page.tracks).await?;
    Ok(page)
}

/// Saves a public Deezer playlist as one of the user's (synced with "Sincronizar").
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_deezer_playlist(app: tauri::AppHandle, id: String) -> CmdResult<PlaylistSummary> {
    let page = app.state::<AppState>().artists.playlist(&id, true).await?;
    with_library(&app, move |l| {
        l.import_playlist(artists::PLAYLIST_ORIGIN, &format!("playlist:{id}"), &page.playlist.title, &page.tracks)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn album_page(app: tauri::AppHandle, id: String) -> CmdResult<AlbumPage> {
    let page = app.state::<AppState>().artists.album(&id).await?;
    remember(&app, &page.tracks).await?;
    Ok(page)
}
