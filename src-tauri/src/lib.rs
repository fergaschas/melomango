mod artists;
mod catalog;
mod discord;
mod downloads;
mod exportify;
mod library;
mod links;
mod lyrics;
mod matcher;
mod mini;
mod radio;
mod models;
mod sources;
mod text;
mod text_import;
mod spotify_export;
mod startup;
mod stream;
mod tray;

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use artists::{AlbumPage, ArtistPage, ArtistService, CatalogSearch, PlaylistPage, Suggestions};
use catalog::{Catalog, RegistryInfo, Registries};
use library::{Download, Library, MonthSummary, Play, Playlist, PlaylistSummary, StoredKind};
use serde::Serialize;
use lyrics::{Lyrics, LyricsService};
use models::{SourceInfo, Track};
use sources::{
    plugin::{self, Collection, LibraryStatus, PluginSource, Requirement},
    MusicSource, Registry,
};
use stream::StreamCache;

struct AppState {
    /// Swapped as a whole when plugins are installed or removed.
    registry: RwLock<Arc<Registry>>,
    /// Every installed plugin, playable or metadata-only.
    plugins: RwLock<Vec<Arc<PluginSource>>>,
    plugins_dir: PathBuf,
    plugin_data_dir: PathBuf,
    registries: Registries,
    library: Library,
    lyrics: LyricsService,
    artists: ArtistService,
    discord: discord::Presence,
    http: reqwest::Client,
    streams: StreamCache,
    downloads: downloads::Downloads,
}

impl AppState {
    fn registry(&self) -> Arc<Registry> {
        self.registry.read().unwrap().clone()
    }

    fn reload(&self) {
        let plugins: Vec<Arc<PluginSource>> = plugin::discover(&self.plugins_dir, &self.plugin_data_dir)
            .into_iter()
            .map(Arc::new)
            .collect();
        let sources: Vec<Arc<dyn MusicSource>> = plugins
            .iter()
            .filter(|p| p.is_source())
            .map(|p| p.clone() as Arc<dyn MusicSource>)
            .collect();
        *self.registry.write().unwrap() = Arc::new(Registry::new(sources));
        *self.plugins.write().unwrap() = plugins;
    }

    fn source(&self, id: &str) -> Result<Arc<dyn MusicSource>, String> {
        self.registry().get(id).map_err(|e| e.to_string())
    }

    fn plugin(&self, id: &str) -> Result<Arc<PluginSource>, String> {
        self.plugins
            .read()
            .unwrap()
            .iter()
            .find(|p| p.id() == id)
            .cloned()
            .ok_or_else(|| format!("el plugin {id} no está instalado"))
    }

    /// Name and songs behind a pasted playlist or album link. `fresh` skips caches (syncing).
    async fn fetch_link(&self, link: &links::Link, fresh: bool) -> anyhow::Result<(String, Vec<Track>)> {
        use links::Link;
        match link {
            Link::SpotifyPlaylist(id) => links::spotify_collection(&self.http, "playlist", id).await,
            Link::SpotifyAlbum(id) => links::spotify_collection(&self.http, "album", id).await,
            Link::YouTubePlaylist(id) => {
                let youtube = self
                    .plugin("youtube")
                    .map_err(|_| anyhow::anyhow!("instala el plugin de YouTube para importar listas de YouTube"))?;
                youtube
                    .playlist(id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("actualiza el plugin de YouTube para importar listas de YouTube"))
            }
            Link::DeezerPlaylist(id) => {
                let page = self.artists.playlist(id, fresh).await?;
                Ok((page.playlist.title, page.tracks))
            }
            Link::DeezerAlbum(id) => {
                let page = self.artists.album(id).await?;
                Ok((page.album.title, page.tracks))
            }
        }
    }

    fn library_plugin(&self, id: &str) -> Result<Arc<PluginSource>, String> {
        let plugin = self.plugin(id)?;
        if !plugin.is_library() {
            return Err(format!("{} no es un plugin de biblioteca", plugin.name()));
        }
        Ok(plugin)
    }
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

/// Runs `f` with the app state on the pool meant for blocking work (SQLite, files).
///
/// A Tauri command without `async` runs on the window's thread: a slow one (importing a
/// Spotify export, emptying the cache) froze the window until Windows said «No responde»,
/// and even a quick one waited there whenever another command held the database. Async
/// commands must not do this work inline either, or they stall the runtime's threads.
async fn blocking<T, F>(app: tauri::AppHandle, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| format!("la tarea se interrumpió: {e}"))?
}

/// `blocking` for library work on owned data, from async commands.
async fn with_library<T, F>(app: &tauri::AppHandle, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&Library) -> anyhow::Result<T> + Send + 'static,
{
    blocking(app.clone(), move |s| f(&s.library).map_err(err)).await
}

#[tauri::command]
fn list_sources(state: State<'_, AppState>) -> Vec<SourceInfo> {
    state.registry().list()
}

/// Every installed plugin (including metadata-only ones).
#[tauri::command]
fn list_plugins(state: State<'_, AppState>) -> Vec<SourceInfo> {
    state.plugins.read().unwrap().iter().map(|p| p.info()).collect()
}

#[tauri::command]
async fn search(state: State<'_, AppState>, source: String, query: String) -> Result<Vec<Track>, String> {
    let source = state.source(&source)?;
    source.search(&query).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn trending(state: State<'_, AppState>, source: String) -> Result<Vec<Track>, String> {
    let source = state.source(&source)?;
    source.trending().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn install_plugin(app: tauri::AppHandle, path: String) -> Result<SourceInfo, String> {
    blocking(app, move |state| {
        let id = plugin::install(path.as_ref(), &state.plugins_dir, None).map_err(err)?;
        state.reload();
        state.plugin(&id).map(|p| p.info())
    })
    .await
}

#[tauri::command]
async fn uninstall_plugin(app: tauri::AppHandle, id: String) -> Result<(), String> {
    blocking(app, move |state| {
        plugin::uninstall(&id, &state.plugins_dir, &state.plugin_data_dir).map_err(err)?;
        state.reload();
        Ok(())
    })
    .await
}

#[tauri::command]
async fn browse_plugins(state: State<'_, AppState>) -> Result<Catalog, String> {
    Ok(catalog::browse(&state.http, &state.registries.list()).await)
}

/// The repository the installed plugin `id` came from, if installing it from `registry`
/// would replace it with another repository's plugin (the app asks first).
#[tauri::command]
async fn plugin_origin_change(app: tauri::AppHandle, registry: String, id: String) -> Result<Option<String>, String> {
    blocking(app, move |state| Ok(catalog::origin_change(&state.plugins_dir, &registry, &id))).await
}

#[tauri::command]
async fn install_catalog_plugin(
    app: tauri::AppHandle,
    registry: String,
    id: String,
    replace_other_origin: Option<bool>,
) -> Result<SourceInfo, String> {
    let state = app.state::<AppState>();
    let id = catalog::install(
        &state.http,
        &registry,
        &id,
        &state.plugins_dir,
        &state.plugin_data_dir,
        replace_other_origin.unwrap_or(false),
    )
    .await
    .map_err(|e| format!("{e:#}"))?;
    blocking(app.clone(), move |state| {
        state.reload();
        state.plugin(&id).map(|p| p.info())
    })
    .await
}

#[tauri::command]
async fn list_registries(app: tauri::AppHandle) -> Result<Vec<RegistryInfo>, String> {
    blocking(app, |state| Ok(state.registries.list())).await
}

#[tauri::command]
async fn add_registry(app: tauri::AppHandle, url: String) -> Result<Vec<RegistryInfo>, String> {
    blocking(app, move |state| {
        state.registries.add(&url).map_err(err)?;
        Ok(state.registries.list())
    })
    .await
}

#[tauri::command]
async fn remove_registry(app: tauri::AppHandle, url: String) -> Result<Vec<RegistryInfo>, String> {
    blocking(app, move |state| {
        state.registries.remove(&url).map_err(err)?;
        Ok(state.registries.list())
    })
    .await
}

#[tauri::command]
fn open_plugins_dir(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    app.opener()
        .open_path(state.plugins_dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_lyrics(state: State<'_, AppState>, track: Track) -> Result<Lyrics, String> {
    state.lyrics.get(&track).await.map_err(|e| format!("{e:#}"))
}

// ---------- Library ----------

#[tauri::command]
async fn liked_tracks(app: tauri::AppHandle) -> Result<Vec<Track>, String> {
    with_library(&app, |l| l.liked_tracks()).await
}

#[tauri::command]
async fn liked_keys(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    with_library(&app, |l| l.liked_keys()).await
}

#[tauri::command]
async fn set_liked(app: tauri::AppHandle, track: Track, liked: bool) -> Result<(), String> {
    with_library(&app, move |l| l.set_liked(&track, liked)).await
}

#[tauri::command]
async fn list_playlists(app: tauri::AppHandle) -> Result<Vec<PlaylistSummary>, String> {
    with_library(&app, |l| l.playlists()).await
}

#[tauri::command]
async fn create_playlist(app: tauri::AppHandle, name: String) -> Result<PlaylistSummary, String> {
    with_library(&app, move |l| l.create_playlist(&name)).await
}

#[tauri::command]
async fn rename_playlist(app: tauri::AppHandle, id: i64, name: String) -> Result<(), String> {
    with_library(&app, move |l| l.rename_playlist(id, &name)).await
}

#[tauri::command]
async fn delete_playlist(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    with_library(&app, move |l| l.delete_playlist(id)).await
}

#[tauri::command]
async fn get_playlist(app: tauri::AppHandle, id: i64) -> Result<Playlist, String> {
    with_library(&app, move |l| l.playlist(id)).await
}

#[tauri::command]
async fn add_to_playlist(app: tauri::AppHandle, id: i64, track: Track) -> Result<bool, String> {
    with_library(&app, move |l| l.add_to_playlist(id, &track)).await
}

#[tauri::command]
async fn remove_from_playlist(app: tauri::AppHandle, id: i64, source: String, track_id: String) -> Result<(), String> {
    with_library(&app, move |l| l.remove_from_playlist(id, &source, &track_id)).await
}

#[tauri::command]
async fn move_in_playlist(app: tauri::AppHandle, id: i64, from: usize, to: usize) -> Result<(), String> {
    with_library(&app, move |l| l.move_in_playlist(id, from, to)).await
}

/// What the plugin needs installed on the computer, and whether it is there.
#[tauri::command]
async fn plugin_requirements(state: State<'_, AppState>, plugin_id: String) -> Result<Vec<Requirement>, String> {
    state.plugin(&plugin_id)?.requirements().await.map_err(err)
}

/// Lets the plugin install a requirement (e.g. yt-dlp with winget).
#[tauri::command]
async fn install_plugin_requirement(
    state: State<'_, AppState>,
    plugin_id: String,
    requirement_id: String,
) -> Result<Vec<Requirement>, String> {
    state.plugin(&plugin_id)?.install_requirement(&requirement_id).await.map_err(err)
}

// ---------- Services (library plugins such as Spotify) ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    plugin_id: String,
    name: String,
    connected: bool,
    user: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionView {
    #[serde(flatten)]
    collection: Collection,
    /// Melomango playlist it was imported into, if any.
    imported_playlist_id: Option<i64>,
}

#[tauri::command]
async fn library_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let plugins: Vec<Arc<PluginSource>> =
        state.plugins.read().unwrap().iter().filter(|p| p.is_library()).cloned().collect();
    let mut connections = Vec::new();
    for p in plugins {
        let (status, error) = match p.library_status().await {
            Ok(s) => (s, None),
            Err(e) => (LibraryStatus { connected: false, user: None }, Some(err(e))),
        };
        connections.push(Connection {
            plugin_id: p.id().to_string(),
            name: p.name().to_string(),
            connected: status.connected,
            user: status.user,
            error,
        });
    }
    Ok(connections)
}

#[tauri::command]
async fn connect_library(state: State<'_, AppState>, plugin_id: String) -> Result<LibraryStatus, String> {
    state.library_plugin(&plugin_id)?.connect().await.map_err(err)
}

#[tauri::command]
async fn disconnect_library(state: State<'_, AppState>, plugin_id: String) -> Result<LibraryStatus, String> {
    state.library_plugin(&plugin_id)?.disconnect().await.map_err(err)
}

#[tauri::command]
async fn library_collections(app: tauri::AppHandle, plugin_id: String) -> Result<Vec<CollectionView>, String> {
    let plugin = app.state::<AppState>().library_plugin(&plugin_id)?;
    let collections = plugin.collections().await.map_err(err)?;
    let imported = with_library(&app, move |l| l.imported_from(&plugin_id)).await?;
    Ok(collections
        .into_iter()
        .map(|c| CollectionView { imported_playlist_id: imported.get(&c.id).copied(), collection: c })
        .collect())
}

/// Copies a collection into a Melomango playlist (or refreshes the one it was imported into).
#[tauri::command]
async fn import_collection(
    app: tauri::AppHandle,
    plugin_id: String,
    collection_id: String,
    name: String,
) -> Result<PlaylistSummary, String> {
    let plugin = app.state::<AppState>().library_plugin(&plugin_id)?;
    let tracks = plugin.collection_tracks(&collection_id).await.map_err(err)?;
    with_library(&app, move |l| l.import_playlist(&plugin_id, &collection_id, &name, &tracks)).await
}

#[tauri::command]
async fn sync_playlist(app: tauri::AppHandle, id: i64) -> Result<PlaylistSummary, String> {
    let link = with_library(&app, move |l| l.playlist_link(id))
        .await?
        .ok_or("esta playlist no está vinculada a ningún servicio")?;
    let state = app.state::<AppState>();
    let tracks = if let Some(source) = links::Link::from_origin(&link.plugin_id, &link.remote_id) {
        state.fetch_link(&source, true).await.map_err(|e| format!("{e:#}"))?.1
    } else {
        state.library_plugin(&link.plugin_id)?.collection_tracks(&link.remote_id).await.map_err(err)?
    };
    with_library(&app, move |l| l.import_playlist(&link.plugin_id, &link.remote_id, "importada", &tracks)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportedLink {
    playlist: PlaylistSummary,
    /// Spotify's public page only lists the first 100 songs.
    limited: bool,
}

/// Imports the playlist or album behind a pasted Spotify, YouTube or Deezer link.
#[tauri::command]
async fn import_link(app: tauri::AppHandle, url: String) -> Result<ImportedLink, String> {
    let link = links::parse(&url).ok_or(
        "no reconozco ese enlace: pega el de una playlist o un álbum de Spotify, YouTube (o YouTube Music) o Deezer",
    )?;
    let (name, tracks) = app.state::<AppState>().fetch_link(&link, true).await.map_err(|e| format!("{e:#}"))?;
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
struct ImportedText {
    playlist: PlaylistSummary,
    /// Lines that weren't songs.
    skipped: usize,
}

/// A new playlist from pasted text, one song per line ("Artista - Título").
#[tauri::command]
async fn import_text(app: tauri::AppHandle, name: String, text: String) -> Result<ImportedText, String> {
    blocking(app, move |state| {
        let parsed = text_import::parse(&text);
        if parsed.tracks.is_empty() {
            return Err("no encontré canciones: pon una por línea, por ejemplo «Vetusta Morla - Copenhague»".into());
        }
        let name = if name.trim().is_empty() { "Lista pegada".to_string() } else { name };
        let playlist = state.library.create_playlist_with(&name, &parsed.tracks).map_err(err)?;
        Ok(ImportedText { playlist, skipped: parsed.skipped })
    })
    .await
}

/// Imports the playlists and liked songs of a Spotify data export (zip or its JSON files).
#[tauri::command]
async fn import_spotify_export(app: tauri::AppHandle, paths: Vec<String>) -> Result<ImportedExport, String> {
    blocking(app, move |state| {
        let export = spotify_export::read(&paths).map_err(err)?;
        let playlists: Vec<(&str, &str, &[Track])> = export
            .collections
            .iter()
            .map(|c| (c.remote_id.as_str(), c.name.as_str(), c.tracks.as_slice()))
            .collect();
        let playlists = state.library.import_many(spotify_export::ORIGIN, &playlists).map_err(err)?;
        Ok(ImportedExport { playlists, problems: export.problems })
    })
    .await
}

#[derive(Serialize)]
struct ImportedExport {
    playlists: Vec<PlaylistSummary>,
    /// Files (or parts of them) that couldn't be read and were skipped, one line each.
    problems: Vec<String>,
}

/// Shrinks the main window to the mini player (always on top), or puts it back.
#[tauri::command]
fn set_mini_mode(app: tauri::AppHandle, mini: State<'_, mini::MiniState>, on: bool) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("no se encontró la ventana principal")?;
    mini.set(&window, on).map_err(|e| e.to_string())
}

/// What the tray icon shows: the song in its tooltip and play/pause in its menu.
#[tauri::command]
fn tray_update(app: tauri::AppHandle, title: Option<String>, playing: bool) {
    if let Some(tray) = app.try_state::<tray::Tray>() {
        tray.update(&app, title.as_deref(), playing);
    }
}

/// Whether closing the window hides it in the tray (and the music keeps playing).
#[tauri::command]
fn set_close_to_tray(app: tauri::AppHandle, on: bool) {
    if let Some(tray) = app.try_state::<tray::Tray>() {
        tray.set_close_to_tray(on);
    }
}

/// Shows what is playing in Discord ("Escuchando Melomango"), or clears it with `null`.
#[tauri::command]
fn discord_set(state: State<'_, AppState>, now: Option<discord::NowPlaying>) {
    state.discord.set(now);
}

#[tauri::command]
fn discord_status(state: State<'_, AppState>) -> discord::Status {
    state.discord.status()
}

/// An artist's page (popular songs, releases, similar artists), by Deezer id or by name.
#[tauri::command]
async fn artist_page(app: tauri::AppHandle, name: String, id: Option<String>) -> Result<ArtistPage, String> {
    let page = app.state::<AppState>().artists.artist(&name, id.as_deref()).await.map_err(|e| format!("{e:#}"))?;
    remember(&app, &page.top).await?;
    Ok(page)
}

/// Stores tracks shown on catalog pages so they can be played (matched to YouTube) later.
async fn remember(app: &tauri::AppHandle, tracks: &[Track]) -> Result<(), String> {
    let tracks = tracks.to_vec();
    with_library(app, move |l| l.remember_tracks(&tracks)).await
}

/// Artists, albums and public playlists matching `query` (from Deezer).
#[tauri::command]
async fn catalog_search(state: State<'_, AppState>, query: String) -> Result<CatalogSearch, String> {
    state.artists.search(&query).await.map_err(|e| format!("{e:#}"))
}

/// Artists and songs to suggest while typing a search (from Deezer).
#[tauri::command]
async fn search_suggestions(app: tauri::AppHandle, query: String) -> Result<Suggestions, String> {
    if query.trim().chars().count() < 2 {
        return Ok(Suggestions { artists: Vec::new(), tracks: Vec::new() });
    }
    let found = app.state::<AppState>().artists.suggest(&query).await.map_err(|e| format!("{e:#}"))?;
    // Suggested songs can be played straight away (matched to YouTube).
    remember(&app, &found.tracks).await?;
    Ok(found)
}

/// A public Deezer playlist with its songs.
#[tauri::command]
async fn deezer_playlist(app: tauri::AppHandle, id: String) -> Result<PlaylistPage, String> {
    let page = app.state::<AppState>().artists.playlist(&id, false).await.map_err(|e| format!("{e:#}"))?;
    remember(&app, &page.tracks).await?;
    Ok(page)
}

/// Saves a public Deezer playlist as one of the user's (synced with "Sincronizar").
#[tauri::command]
async fn save_deezer_playlist(app: tauri::AppHandle, id: String) -> Result<PlaylistSummary, String> {
    let page = app.state::<AppState>().artists.playlist(&id, true).await.map_err(|e| format!("{e:#}"))?;
    with_library(&app, move |l| {
        l.import_playlist(artists::PLAYLIST_ORIGIN, &format!("playlist:{id}"), &page.playlist.title, &page.tracks)
    })
    .await
}

#[tauri::command]
async fn album_page(app: tauri::AppHandle, id: String) -> Result<AlbumPage, String> {
    let page = app.state::<AppState>().artists.album(&id).await.map_err(|e| format!("{e:#}"))?;
    remember(&app, &page.tracks).await?;
    Ok(page)
}

/// Songs like `track` for the automatic radio, leaving out `exclude` ("source/id" already queued).
#[tauri::command]
async fn radio_tracks(state: State<'_, AppState>, track: Track, exclude: Vec<String>) -> Result<Vec<Track>, String> {
    let registry = state.registry();
    let exclude = exclude.into_iter().collect();
    radio::tracks(&registry, &state.library, &state.http, &track, &exclude)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// Records that the user listened to `track` (called once it has played a while).
#[tauri::command]
async fn record_play(app: tauri::AppHandle, track: Track) -> Result<(), String> {
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
async fn recent_plays(app: tauri::AppHandle, limit: Option<u32>) -> Result<Vec<Play>, String> {
    with_library(&app, move |l| l.recent_plays(limit.unwrap_or(500).min(5_000))).await
}

/// Months with plays ("YYYY-MM"), newest first.
#[tauri::command]
async fn summary_months(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    with_library(&app, |l| l.summary_months()).await
}

/// "Tu resumen del mes": totals and favourites of `month` ("YYYY-MM").
#[tauri::command]
async fn month_summary(app: tauri::AppHandle, month: String) -> Result<MonthSummary, String> {
    with_library(&app, move |l| l.month_summary(&month)).await
}

#[tauri::command]
async fn clear_history(app: tauri::AppHandle) -> Result<(), String> {
    with_library(&app, |l| l.clear_history()).await
}

/// Recordings to choose from when an imported track plays the wrong one.
#[tauri::command]
async fn match_options(
    state: State<'_, AppState>,
    source: String,
    id: String,
    query: Option<String>,
) -> Result<matcher::MatchOptions, String> {
    let registry = state.registry();
    matcher::options(&registry, &state.library, &source, &id, query).await.map_err(|e| format!("{e:#}"))
}

/// Plays `source/id` from the recording the user chose from now on.
#[tauri::command]
async fn choose_match(
    app: tauri::AppHandle,
    source: String,
    id: String,
    play_source: String,
    play_id: String,
) -> Result<(), String> {
    blocking(app, move |state| {
        state.source(&play_source)?;
        state.library.set_track_match(&source, &id, &play_source, &play_id).map_err(err)?;
        state.streams.forget(&format!("{source}/{id}"));
        // A download of the old recording would keep playing it.
        state.downloads.remove(&state.library, &source, &id).map_err(err)
    })
    .await
}

// ---------- Downloads ----------

const DOWNLOAD_EVENT: &str = "download-progress";

/// Downloads songs in the background; progress arrives as `download-progress` events.
#[tauri::command]
fn download_tracks(app: tauri::AppHandle, tracks: Vec<Track>) {
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
async fn remove_download(app: tauri::AppHandle, source: String, id: String) -> Result<(), String> {
    blocking(app, move |state| state.downloads.remove(&state.library, &source, &id).map_err(err)).await
}

#[tauri::command]
async fn list_downloads(app: tauri::AppHandle) -> Result<Vec<Download>, String> {
    with_library(&app, |l| l.downloads()).await
}

#[tauri::command]
async fn downloaded_keys(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    with_library(&app, |l| l.downloaded_keys()).await
}

/// Bytes the automatic cache may use (0 turns it off and empties it).
#[tauri::command]
async fn set_cache_limit(app: tauri::AppHandle, bytes: u64) -> Result<(), String> {
    blocking(app, move |state| state.downloads.set_cache_limit(&state.library, bytes).map_err(err)).await
}

#[derive(Serialize)]
struct CacheUsage {
    songs: u32,
    bytes: u64,
}

#[tauri::command]
async fn cache_usage(app: tauri::AppHandle) -> Result<CacheUsage, String> {
    let (songs, bytes) = with_library(&app, |l| l.cache_usage()).await?;
    Ok(CacheUsage { songs, bytes })
}

#[tauri::command]
async fn clear_cache(app: tauri::AppHandle) -> Result<(), String> {
    blocking(app, |state| state.downloads.clear_cache(&state.library).map_err(err)).await
}

/// Resolves a track's audio in the background so it starts at once when played.
#[tauri::command]
fn prefetch_stream(app: tauri::AppHandle, source: String, id: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let registry = state.registry();
        // Best effort: a failure here is retried (and reported) when the track is played.
        let _ = stream::prefetch(&registry, &state.library, &state.http, &state.streams, &source, &id).await;
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Opening Melomango again (e.g. while it plays in the tray) shows the running one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let plugins_dir = data_dir.join("plugins");
            std::fs::create_dir_all(&plugins_dir)?;
            plugin::clean_leftovers(&plugins_dir);
            let plugin_data_dir = data_dir.join("plugin-data");

            // Without timeouts a server that hangs leaves a stream, a download or the lyrics
            // waiting forever. The read timeout counts between two pieces of an answer, not
            // the whole answer, so long downloads keep going while data flows.
            let http = reqwest::Client::builder()
                .user_agent(concat!("Melomango/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(10))
                .read_timeout(std::time::Duration::from_secs(30))
                .build()?;
            let library = match Library::open(&data_dir.join("library.db")) {
                Ok(library) => library,
                Err(e) => {
                    // Explain it (and update, if the library is newer) instead of crashing.
                    startup::library_failed(app.handle(), e);
                    return Ok(());
                }
            };
            let (discord, discord_task) = discord::Presence::new(discord::APPLICATION_ID);
            tauri::async_runtime::spawn(discord_task);
            let state = AppState {
                registry: Default::default(),
                plugins: Default::default(),
                plugins_dir,
                plugin_data_dir,
                registries: Registries::new(data_dir.join("registries.json")),
                library,
                lyrics: LyricsService::new(http.clone()),
                artists: ArtistService::new(http.clone()),
                discord,
                http,
                streams: StreamCache::default(),
                downloads: downloads::Downloads::new(data_dir.join("downloads")),
            };
            state.reload();
            // What an earlier run left half done (it closed mid-download).
            state.downloads.clean_orphans(&state.library);
            app.manage(state);
            app.manage(mini::MiniState::default());
            // Without a tray (some Linux desktops) the app works as before.
            match tray::Tray::create(app.handle()) {
                Ok(tray) => {
                    app.manage(tray);
                }
                Err(e) => eprintln!("sin icono en la bandeja: {e}"),
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window.app_handle().try_state::<tray::Tray>().is_some_and(|t| t.closes_to_tray());
                if window.label() == "main" && to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .register_asynchronous_uri_scheme_protocol("stream", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                // Missing only if the app couldn't start (see `startup`).
                let Some(state) = app.try_state::<AppState>() else {
                    responder.respond(tauri::http::Response::builder().status(503).body(Vec::new()).unwrap());
                    return;
                };
                let response = stream::handle(
                    state.registry(),
                    &state.library,
                    &state.http,
                    &state.streams,
                    &state.downloads,
                    request,
                )
                .await;
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            list_sources,
            list_plugins,
            plugin_requirements,
            install_plugin_requirement,
            library_connections,
            connect_library,
            disconnect_library,
            library_collections,
            import_collection,
            sync_playlist,
            search,
            trending,
            install_plugin,
            uninstall_plugin,
            browse_plugins,
            install_catalog_plugin,
            plugin_origin_change,
            list_registries,
            add_registry,
            remove_registry,
            open_plugins_dir,
            liked_tracks,
            liked_keys,
            set_liked,
            list_playlists,
            create_playlist,
            rename_playlist,
            delete_playlist,
            get_playlist,
            add_to_playlist,
            remove_from_playlist,
            move_in_playlist,
            get_lyrics,
            import_spotify_export,
            import_link,
            import_text,
            match_options,
            choose_match,
            record_play,
            radio_tracks,
            artist_page,
            album_page,
            catalog_search,
            search_suggestions,
            deezer_playlist,
            save_deezer_playlist,
            discord_set,
            discord_status,
            tray_update,
            set_mini_mode,
            download_tracks,
            remove_download,
            list_downloads,
            downloaded_keys,
            set_cache_limit,
            cache_usage,
            clear_cache,
            set_close_to_tray,
            recent_plays,
            clear_history,
            summary_months,
            month_summary,
            prefetch_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
