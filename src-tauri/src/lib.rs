mod artists;
mod catalog;
mod commands;
mod discord;
mod downloads;
mod exportify;
mod library;
mod links;
mod lyrics;
mod matcher;
mod mini;
mod models;
mod radio;
mod sources;
mod spotify_export;
mod startup;
mod stations;
mod stream;
mod text;
mod text_import;
mod tray;

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use artists::{AlbumPage, ArtistPage, ArtistService, CatalogSearch, PlaylistPage, Suggestions};
use catalog::{Catalog, Registries, RegistryInfo};
use library::{Download, Library, MonthSummary, Play, Playlist, PlaylistSummary, StoredKind};
use lyrics::{Lyrics, LyricsService};
use models::{SourceInfo, Track};
use sources::{
    MusicSource, Registry,
    plugin::{self, Collection, LibraryStatus, PluginSource, Requirement},
};
use stream::StreamCache;

/// What is installed: swapped as a whole when plugins are installed or removed. One lock
/// for both, so nobody reads a new registry together with the old list of plugins.
#[derive(Default)]
struct Loaded {
    registry: Arc<Registry>,
    /// Every installed plugin, playable or metadata-only.
    plugins: Vec<Arc<PluginSource>>,
}

struct AppState {
    loaded: RwLock<Loaded>,
    plugins_dir: PathBuf,
    plugin_data_dir: PathBuf,
    registries: Registries,
    library: Library,
    lyrics: LyricsService,
    artists: ArtistService,
    discord: discord::Presence,
    http: reqwest::Client,
    /// Only https: for plugin repositories and downloads (see `catalog::client`).
    catalog_http: reqwest::Client,
    streams: StreamCache,
    downloads: downloads::Downloads,
    /// The Radio Browser directory, for «Emisoras».
    stations: stations::RadioBrowser,
    /// The station playing now (through `stream://localhost/_live/<session>`).
    live: stations::live::LiveStreams,
}

impl AppState {
    fn registry(&self) -> Arc<Registry> {
        self.loaded.read().unwrap().registry.clone()
    }

    /// Every installed plugin (a snapshot).
    fn plugins(&self) -> Vec<Arc<PluginSource>> {
        self.loaded.read().unwrap().plugins.clone()
    }

    fn reload(&self) {
        let plugins: Vec<Arc<PluginSource>> =
            plugin::discover(&self.plugins_dir, &self.plugin_data_dir).into_iter().map(Arc::new).collect();
        let sources: Vec<Arc<dyn MusicSource>> =
            plugins.iter().filter(|p| p.is_source()).map(|p| p.clone() as Arc<dyn MusicSource>).collect();
        *self.loaded.write().unwrap() = Loaded { registry: Arc::new(Registry::new(sources)), plugins };
    }

    fn source(&self, id: &str) -> Result<Arc<dyn MusicSource>, String> {
        self.registry().get(id).map_err(|e| e.to_string())
    }

    fn plugin(&self, id: &str) -> Result<Arc<PluginSource>, String> {
        self.loaded
            .read()
            .unwrap()
            .plugins
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

/// Every command the frontend can call, for Tauri and for the generated TypeScript
/// bindings (`src/lib/bindings.ts`, see `export_bindings`): one list for both.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::plugins::list_sources,
            commands::plugins::list_plugins,
            commands::plugins::plugin_requirements,
            commands::plugins::install_plugin_requirement,
            commands::services::library_connections,
            commands::services::connect_library,
            commands::services::disconnect_library,
            commands::services::library_collections,
            commands::services::import_collection,
            commands::services::sync_playlist,
            commands::plugins::search,
            commands::plugins::trending,
            commands::plugins::install_plugin,
            commands::plugins::uninstall_plugin,
            commands::plugins::browse_plugins,
            commands::plugins::install_catalog_plugin,
            commands::plugins::plugin_origin_change,
            commands::plugins::list_registries,
            commands::plugins::add_registry,
            commands::plugins::remove_registry,
            commands::plugins::open_plugins_dir,
            commands::library::liked_tracks,
            commands::library::liked_keys,
            commands::library::set_liked,
            commands::library::list_playlists,
            commands::library::create_playlist,
            commands::library::rename_playlist,
            commands::library::delete_playlist,
            commands::library::get_playlist,
            commands::library::add_to_playlist,
            commands::library::remove_from_playlist,
            commands::library::move_in_playlist,
            commands::pages::get_lyrics,
            commands::services::import_spotify_export,
            commands::services::import_link,
            commands::services::import_text,
            commands::library::match_options,
            commands::library::choose_match,
            commands::library::record_play,
            commands::library::radio_tracks,
            commands::pages::artist_page,
            commands::pages::album_page,
            commands::pages::catalog_search,
            commands::pages::search_suggestions,
            commands::pages::deezer_playlist,
            commands::pages::save_deezer_playlist,
            commands::app::discord_set,
            commands::app::discord_status,
            commands::app::tray_update,
            commands::app::set_mini_mode,
            commands::downloads::download_tracks,
            commands::downloads::remove_download,
            commands::downloads::list_downloads,
            commands::downloads::downloaded_keys,
            commands::downloads::set_cache_limit,
            commands::downloads::cache_usage,
            commands::downloads::clear_cache,
            commands::app::set_close_to_tray,
            commands::library::recent_plays,
            commands::library::clear_history,
            commands::library::summary_months,
            commands::library::month_summary,
            commands::downloads::prefetch_stream,
            commands::stations::stations_near,
            commands::stations::stations_top,
            commands::stations::stations_search,
            commands::stations::station_countries,
            commands::stations::stations_liked,
            commands::stations::stations_like,
            commands::stations::station_play,
            commands::stations::station_stop,
            commands::stations::system_location,
            commands::stations::system_country,
        ])
        // Pushed as an event, not returned by a command: exported so the frontend shares it.
        .typ::<downloads::Progress>()
        .typ::<stations::live::LiveTitle>()
        // Sizes, millisecond timestamps and SQLite ids, all far below 2^53: plain numbers.
        .dangerously_cast_bigints_to_number()
    // Types whose reading and writing differ (fields with #[serde(default)]) come out twice:
    // `X_Serialize` is what the frontend receives (every field present), `X_Deserialize`
    // what a command accepts. api.ts names the first one plainly (`Track`).
}

/// Writes the TypeScript bindings to `path` (`cargo test` keeps the committed file current).
#[cfg(test)]
fn export_bindings(path: &std::path::Path) {
    specta_builder()
        .export(specta_typescript::Typescript::default(), path)
        .expect("no se pudieron generar los tipos de TypeScript");
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
                loaded: Default::default(),
                plugins_dir,
                plugin_data_dir,
                registries: Registries::new(data_dir.join("registries.json")),
                library,
                lyrics: LyricsService::new(http.clone()),
                artists: ArtistService::new(http.clone()),
                stations: stations::RadioBrowser::new(http.clone()),
                live: Default::default(),
                discord,
                http,
                catalog_http: catalog::client(concat!("Melomango/", env!("CARGO_PKG_VERSION")))?,
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
                // A live radio station, or a song.
                if let Some(session) = stations::live::session_path(&request) {
                    responder.respond(state.live.handle(&session, &request).await);
                    return;
                }
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
        .invoke_handler(specta_builder().invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod bindings_tests {
    use std::path::Path;

    /// The committed `src/lib/bindings.ts` matches the Rust commands and types. On a change
    /// this test rewrites it, so it fails once and the new file is ready to commit; the CI
    /// (which starts from the committed file) fails when someone forgot to.
    #[test]
    fn typescript_bindings_are_up_to_date() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/bindings.ts");
        let before = std::fs::read_to_string(&path).unwrap_or_default();
        super::export_bindings(&path);
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            before.replace("\r\n", "\n") == after.replace("\r\n", "\n"),
            "src/lib/bindings.ts estaba desactualizado y se acaba de regenerar: revísalo y súbelo"
        );
    }
}
