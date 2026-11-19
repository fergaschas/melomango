mod catalog;
mod library;
mod lyrics;
mod models;
mod sources;
mod stream;

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

use catalog::{Catalog, RegistryInfo, Registries};
use library::{Library, Playlist, PlaylistSummary};
use lyrics::{Lyrics, LyricsService};
use models::{SourceInfo, Track};
use sources::{audius::Audius, plugin, MusicSource, Registry};
use stream::StreamCache;

struct AppState {
    /// Swapped as a whole when plugins are installed or removed.
    registry: RwLock<Arc<Registry>>,
    builtins: Vec<Arc<dyn MusicSource>>,
    plugins_dir: PathBuf,
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
        let mut sources = self.builtins.clone();
        sources.extend(
            plugin::discover(&self.plugins_dir)
                .into_iter()
                .map(|p| Arc::new(p) as Arc<dyn MusicSource>),
        );
        *self.registry.write().unwrap() = Arc::new(Registry::new(sources));
    }

    fn source(&self, id: &str) -> Result<Arc<dyn MusicSource>, String> {
        self.registry().get(id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn list_sources(state: State<'_, AppState>) -> Vec<SourceInfo> {
    state.registry().list()
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
    state.registry().get(&id).map(|s| s.info()).map_err(|e| e.to_string())
}

#[tauri::command]
fn uninstall_plugin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    plugin::uninstall(&id, &state.plugins_dir).map_err(|e| format!("{e:#}"))?;
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
    state.registry().get(&id).map(|s| s.info()).map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let plugins_dir = data_dir.join("plugins");
            std::fs::create_dir_all(&plugins_dir)?;

            let http = reqwest::Client::builder()
                .user_agent(concat!("Melomango/", env!("CARGO_PKG_VERSION")))
                .build()?;
            let state = AppState {
                registry: Default::default(),
                builtins: vec![Arc::new(Audius::new(http.clone()))],
                plugins_dir,
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
                    stream::handle(state.registry(), &state.http, &state.streams, request).await;
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            list_sources,
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
            get_lyrics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
