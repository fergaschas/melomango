//! Library plugins (Spotify…) and imports: links, pasted text, data exports.

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Connection {
    plugin_id: String,
    name: String,
    connected: bool,
    user: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionView {
    #[serde(flatten)]
    collection: Collection,
    /// Melomango playlist it was imported into, if any.
    imported_playlist_id: Option<i64>,
}

#[tauri::command]
pub(crate) async fn library_connections(state: State<'_, AppState>) -> CmdResult<Vec<Connection>> {
    // All at once: each asks its plugin (up to 60 s), one after another added up.
    let mut asking = tokio::task::JoinSet::new();
    for (order, p) in state.plugins().into_iter().filter(|p| p.is_library()).enumerate() {
        asking.spawn(async move {
            let (status, error) = match p.library_status().await {
                Ok(s) => (s, None),
                Err(e) => (LibraryStatus { connected: false, user: None }, Some(format!("{e:#}"))),
            };
            let connection = Connection {
                plugin_id: p.id().to_string(),
                name: p.name().to_string(),
                connected: status.connected,
                user: status.user,
                error,
            };
            (order, connection)
        });
    }
    let mut answers = asking.join_all().await;
    answers.sort_by_key(|(order, _)| *order);
    Ok(answers.into_iter().map(|(_, c)| c).collect())
}

#[tauri::command]
pub(crate) async fn connect_library(state: State<'_, AppState>, plugin_id: String) -> CmdResult<LibraryStatus> {
    state.library_plugin(&plugin_id)?.connect().await.map_err(CmdError::from)
}

#[tauri::command]
pub(crate) async fn disconnect_library(state: State<'_, AppState>, plugin_id: String) -> CmdResult<LibraryStatus> {
    state.library_plugin(&plugin_id)?.disconnect().await.map_err(CmdError::from)
}

#[tauri::command]
pub(crate) async fn library_collections(app: tauri::AppHandle, plugin_id: String) -> CmdResult<Vec<CollectionView>> {
    let plugin = app.state::<AppState>().library_plugin(&plugin_id)?;
    let collections = plugin.collections().await?;
    let imported = with_library(&app, move |l| l.imported_from(&plugin_id)).await?;
    Ok(collections
        .into_iter()
        .map(|c| CollectionView { imported_playlist_id: imported.get(&c.id).copied(), collection: c })
        .collect())
}

/// Copies a collection into a Melomango playlist (or refreshes the one it was imported into).
#[tauri::command]
pub(crate) async fn import_collection(
    app: tauri::AppHandle,
    plugin_id: String,
    collection_id: String,
    name: String,
) -> CmdResult<PlaylistSummary> {
    let plugin = app.state::<AppState>().library_plugin(&plugin_id)?;
    let tracks = plugin.collection_tracks(&collection_id).await?;
    with_library(&app, move |l| l.import_playlist(&plugin_id, &collection_id, &name, &tracks)).await
}

#[tauri::command]
pub(crate) async fn sync_playlist(app: tauri::AppHandle, id: i64) -> CmdResult<PlaylistSummary> {
    let link = with_library(&app, move |l| l.playlist_link(id))
        .await?
        .ok_or("esta playlist no está vinculada a ningún servicio")?;
    let state = app.state::<AppState>();
    let tracks = if let Some(source) = links::Link::from_origin(&link.plugin_id, &link.remote_id) {
        state.fetch_link(&source, true).await?.1
    } else {
        state.library_plugin(&link.plugin_id)?.collection_tracks(&link.remote_id).await?
    };
    with_library(&app, move |l| l.import_playlist(&link.plugin_id, &link.remote_id, "importada", &tracks)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedLink {
    playlist: PlaylistSummary,
    /// Spotify's public page only lists the first 100 songs.
    limited: bool,
}

/// Imports the playlist or album behind a pasted Spotify, YouTube or Deezer link.
#[tauri::command]
pub(crate) async fn import_link(app: tauri::AppHandle, url: String) -> CmdResult<ImportedLink> {
    let link = links::parse(&url).ok_or(
        "no reconozco ese enlace: pega el de una playlist o un álbum de Spotify, YouTube (o YouTube Music) o Deezer",
    )?;
    let (name, tracks) = app.state::<AppState>().fetch_link(&link, true).await?;
    if tracks.is_empty() {
        return Err("esa playlist está vacía".into());
    }
    let limited = matches!(link, links::Link::SpotifyPlaylist(_)) && tracks.len() >= links::SPOTIFY_EMBED_LIMIT;
    let (origin, remote_id) = link.origin();
    let origin = origin.to_string();
    let playlist = with_library(&app, move |l| l.import_playlist(&origin, &remote_id, &name, &tracks)).await?;
    Ok(ImportedLink { playlist, limited })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedText {
    playlist: PlaylistSummary,
    /// Lines that weren't songs.
    skipped: usize,
}

/// A new playlist from pasted text, one song per line ("Artista - Título").
#[tauri::command]
pub(crate) async fn import_text(app: tauri::AppHandle, name: String, text: String) -> CmdResult<ImportedText> {
    blocking(app, move |state| {
        let parsed = text_import::parse(&text);
        if parsed.tracks.is_empty() {
            return Err("no encontré canciones: pon una por línea, por ejemplo «Vetusta Morla - Copenhague»".into());
        }
        let name = if name.trim().is_empty() { "Lista pegada".to_string() } else { name };
        let playlist = state.library.create_playlist_with(&name, &parsed.tracks)?;
        Ok(ImportedText { playlist, skipped: parsed.skipped })
    })
    .await
}

/// Imports the playlists and liked songs of a Spotify data export (zip or its JSON files).
#[tauri::command]
pub(crate) async fn import_spotify_export(app: tauri::AppHandle, paths: Vec<String>) -> CmdResult<ImportedExport> {
    blocking(app, move |state| {
        let export = spotify_export::read(&paths)?;
        let playlists: Vec<(&str, &str, &[Track])> =
            export.collections.iter().map(|c| (c.remote_id.as_str(), c.name.as_str(), c.tracks.as_slice())).collect();
        let playlists = state.library.import_many(spotify_export::ORIGIN, &playlists)?;
        Ok(ImportedExport { playlists, problems: export.problems })
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct ImportedExport {
    playlists: Vec<PlaylistSummary>,
    /// Files (or parts of them) that couldn't be read and were skipped, one line each.
    problems: Vec<String>,
}
