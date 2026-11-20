mod catalog;
mod exportify;
mod library;
mod lyrics;
mod matcher;
mod models;
mod sources;
mod spotify_export;
mod stream;

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

use catalog::{Catalog, RegistryInfo, Registries};
use library::{Library, Playlist, PlaylistSummary};
use serde::Serialize;
use lyrics::{Lyrics, LyricsService};
use models::{SourceInfo, Track};
use sources::{
    audius::Audius,
    plugin::{self, Collection, LibraryStatus, PluginSource, Requirement},
    MusicSource, Registry,
};
use stream::StreamCache;

struct AppState {
    /// Swapped as a whole when plugins are installed or removed.
    registry: RwLock<Arc<Registry>>,
    /// Every installed plugin, playable or metadata-only.
    plugins: RwLock<Vec<Arc<PluginSource>>>,
    builtins: Vec<Arc<dyn MusicSource>>,
    plugins_dir: PathBuf,
    plugin_data_dir: PathBuf,
    registries: Registries,
    library: Library,
    lyrics: LyricsService,
    http: reqwest::Client,
    streams: StreamCache,
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
        let mut sources = self.builtins.clone();
        sources.extend(
            plugins
                .iter()
                .filter(|p| p.is_source())
                .map(|p| p.clone() as Arc<dyn MusicSource>),
        );
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

#[tauri::command]
fn list_sources(state: State<'_, AppState>) -> Vec<SourceInfo> {
    state.registry().list()
}

/// Built-in sources plus every installed plugin (including metadata-only ones).
#[tauri::command]
fn list_plugins(state: State<'_, AppState>) -> Vec<SourceInfo> {
    let mut all: Vec<SourceInfo> = state.builtins.iter().map(|s| s.info()).collect();
    all.extend(state.plugins.read().unwrap().iter().map(|p| p.info()));
    all
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
fn install_plugin(state: State<'_, AppState>, path: String) -> Result<SourceInfo, String> {
    let id = plugin::install(path.as_ref(), &state.plugins_dir).map_err(|e| format!("{e:#}"))?;
    state.reload();
    state.plugin(&id).map(|p| p.info())
}

#[tauri::command]
fn uninstall_plugin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    plugin::uninstall(&id, &state.plugins_dir, &state.plugin_data_dir).map_err(|e| format!("{e:#}"))?;
    state.reload();
    Ok(())
}

#[tauri::command]
async fn browse_plugins(state: State<'_, AppState>) -> Result<Catalog, String> {
    Ok(catalog::browse(&state.http, &state.registries.list()).await)
}

#[tauri::command]
async fn install_catalog_plugin(
    state: State<'_, AppState>,
    registry: String,
    id: String,
) -> Result<SourceInfo, String> {
    let id = catalog::install(&state.http, &registry, &id, &state.plugins_dir)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state.reload();
    state.plugin(&id).map(|p| p.info())
}

#[tauri::command]
fn list_registries(state: State<'_, AppState>) -> Vec<RegistryInfo> {
    state.registries.list()
}

#[tauri::command]
fn add_registry(state: State<'_, AppState>, url: String) -> Result<Vec<RegistryInfo>, String> {
    state.registries.add(&url).map_err(|e| format!("{e:#}"))?;
    Ok(state.registries.list())
}

#[tauri::command]
fn remove_registry(state: State<'_, AppState>, url: String) -> Result<Vec<RegistryInfo>, String> {
    state.registries.remove(&url).map_err(|e| format!("{e:#}"))?;
    Ok(state.registries.list())
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

fn lib_err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
fn liked_tracks(state: State<'_, AppState>) -> Result<Vec<Track>, String> {
    state.library.liked_tracks().map_err(lib_err)
}

#[tauri::command]
fn liked_keys(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.library.liked_keys().map_err(lib_err)
}

#[tauri::command]
fn set_liked(state: State<'_, AppState>, track: Track, liked: bool) -> Result<(), String> {
    state.library.set_liked(&track, liked).map_err(lib_err)
}

#[tauri::command]
fn list_playlists(state: State<'_, AppState>) -> Result<Vec<PlaylistSummary>, String> {
    state.library.playlists().map_err(lib_err)
}

#[tauri::command]
fn create_playlist(state: State<'_, AppState>, name: String) -> Result<PlaylistSummary, String> {
    state.library.create_playlist(&name).map_err(lib_err)
}

#[tauri::command]
fn rename_playlist(state: State<'_, AppState>, id: i64, name: String) -> Result<(), String> {
    state.library.rename_playlist(id, &name).map_err(lib_err)
}

#[tauri::command]
fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.library.delete_playlist(id).map_err(lib_err)
}

#[tauri::command]
fn get_playlist(state: State<'_, AppState>, id: i64) -> Result<Playlist, String> {
    state.library.playlist(id).map_err(lib_err)
}

#[tauri::command]
fn add_to_playlist(state: State<'_, AppState>, id: i64, track: Track) -> Result<bool, String> {
    state.library.add_to_playlist(id, &track).map_err(lib_err)
}

#[tauri::command]
fn remove_from_playlist(
    state: State<'_, AppState>,
    id: i64,
    source: String,
    track_id: String,
) -> Result<(), String> {
    state.library.remove_from_playlist(id, &source, &track_id).map_err(lib_err)
}

#[tauri::command]
fn move_in_playlist(state: State<'_, AppState>, id: i64, from: usize, to: usize) -> Result<(), String> {
    state.library.move_in_playlist(id, from, to).map_err(lib_err)
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
async fn library_collections(state: State<'_, AppState>, plugin_id: String) -> Result<Vec<CollectionView>, String> {
    let collections = state.library_plugin(&plugin_id)?.collections().await.map_err(err)?;
    let imported = state.library.imported_from(&plugin_id).map_err(err)?;
    Ok(collections
        .into_iter()
        .map(|c| CollectionView { imported_playlist_id: imported.get(&c.id).copied(), collection: c })
        .collect())
}

/// Copies a collection into a Melomango playlist (or refreshes the one it was imported into).
#[tauri::command]
async fn import_collection(
    state: State<'_, AppState>,
    plugin_id: String,
    collection_id: String,
    name: String,
) -> Result<PlaylistSummary, String> {
    let tracks = state.library_plugin(&plugin_id)?.collection_tracks(&collection_id).await.map_err(err)?;
    state.library.import_playlist(&plugin_id, &collection_id, &name, &tracks).map_err(err)
}

#[tauri::command]
async fn sync_playlist(state: State<'_, AppState>, id: i64) -> Result<PlaylistSummary, String> {
    let link = state
        .library
        .playlist_link(id)
        .map_err(err)?
        .ok_or("esta playlist no está vinculada a ningún servicio")?;
    let tracks = state.library_plugin(&link.plugin_id)?.collection_tracks(&link.remote_id).await.map_err(err)?;
    state.library.import_playlist(&link.plugin_id, &link.remote_id, "importada", &tracks).map_err(err)
}

/// Imports the playlists and liked songs of a Spotify data export (zip or its JSON files).
#[tauri::command]
fn import_spotify_export(state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<PlaylistSummary>, String> {
    let collections = spotify_export::read(&paths).map_err(err)?;
    collections
        .iter()
        .map(|c| {
            state
                .library
                .import_playlist(spotify_export::ORIGIN, &c.remote_id, &c.name, &c.tracks)
                .map_err(err)
        })
        .collect()
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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let plugins_dir = data_dir.join("plugins");
            std::fs::create_dir_all(&plugins_dir)?;
            let plugin_data_dir = data_dir.join("plugin-data");

            let http = reqwest::Client::builder()
                .user_agent(concat!("Melomango/", env!("CARGO_PKG_VERSION")))
                .build()?;
            let state = AppState {
                registry: Default::default(),
                plugins: Default::default(),
                builtins: vec![Arc::new(Audius::new(http.clone()))],
                plugins_dir,
                plugin_data_dir,
                registries: Registries::new(data_dir.join("registries.json")),
                library: Library::open(&data_dir.join("library.db"))?,
                lyrics: LyricsService::new(http.clone()),
                http,
                streams: StreamCache::default(),
            };
            state.reload();
            app.manage(state);
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("stream", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let response =
                    stream::handle(state.registry(), &state.library, &state.http, &state.streams, request).await;
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
            prefetch_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
