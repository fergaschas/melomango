mod models;
mod sources;
mod stream;

use std::sync::Arc;

use tauri::{Manager, State};

use models::Track;
use sources::{audius::Audius, Registry};
use stream::StreamCache;

struct AppState {
    registry: Registry,
    http: reqwest::Client,
    streams: StreamCache,
}

#[tauri::command]
async fn search(state: State<'_, AppState>, source: String, query: String) -> Result<Vec<Track>, String> {
    let source = state.registry.get(&source).map_err(|e| e.to_string())?;
    source.search(&query).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn trending(state: State<'_, AppState>, source: String) -> Result<Vec<Track>, String> {
    let source = state.registry.get(&source).map_err(|e| e.to_string())?;
    source.trending().await.map_err(|e| format!("{e:#}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let http = reqwest::Client::builder()
        .user_agent(concat!("Melomango/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("no se pudo crear el cliente HTTP");

    let state = AppState {
        registry: Registry::new(vec![Arc::new(Audius::new(http.clone()))]),
        http,
        streams: StreamCache::default(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .register_asynchronous_uri_scheme_protocol("stream", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let response =
                    stream::handle(&state.registry, &state.http, &state.streams, request).await;
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![search, trending])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
