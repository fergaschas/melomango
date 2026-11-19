mod models;
mod sources;
mod stream;

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

use models::{SourceInfo, Track};
use sources::{audius::Audius, plugin, MusicSource, Registry};
use stream::StreamCache;

struct AppState {
    /// Swapped as a whole when plugins are installed or removed.
    registry: RwLock<Arc<Registry>>,
    builtins: Vec<Arc<dyn MusicSource>>,
    plugins_dir: PathBuf,
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
fn open_plugins_dir(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    app.opener()
        .open_path(state.plugins_dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let plugins_dir = app.path().app_data_dir()?.join("plugins");
            std::fs::create_dir_all(&plugins_dir)?;

            let http = reqwest::Client::builder()
                .user_agent(concat!("Melomango/", env!("CARGO_PKG_VERSION")))
                .build()?;
            let state = AppState {
                registry: Default::default(),
                builtins: vec![Arc::new(Audius::new(http.clone()))],
                plugins_dir,
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
            open_plugins_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
