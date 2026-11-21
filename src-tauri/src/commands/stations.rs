//! Live radio stations («Emisoras»): the directory, favourites, where the user is and
//! playing one (docs/emisoras.md).

use super::*;
use crate::stations::{
    Country, Station,
    live::{LiveTitle, TitleSink},
    location::{self, Coords},
};

/// The event a playing station's «now playing» comes in.
pub(crate) const LIVE_TITLE_EVENT: &str = "live-title";

/// «Cerca de ti»: stations within 50 km of `lat`, `long` (rounded to ~1 km before asking).
#[tauri::command]
#[specta::specta]
pub(crate) async fn stations_near(state: State<'_, AppState>, lat: f64, long: f64) -> CmdResult<Vec<Station>> {
    state.stations.near(lat, long).await.map_err(directory_error)
}

/// The most listened stations of `country` ("ES").
#[tauri::command]
#[specta::specta]
pub(crate) async fn stations_top(state: State<'_, AppState>, country: String) -> CmdResult<Vec<Station>> {
    state.stations.top(&country).await.map_err(directory_error)
}

/// Stations whose name has every word of `query` or, failing that, of that genre.
#[tauri::command]
#[specta::specta]
pub(crate) async fn stations_search(
    state: State<'_, AppState>,
    query: String,
    country: Option<String>,
) -> CmdResult<Vec<Station>> {
    state.stations.search(&query, country.as_deref()).await.map_err(directory_error)
}

/// Countries with stations, for the selector.
#[tauri::command]
#[specta::specta]
pub(crate) async fn station_countries(state: State<'_, AppState>) -> CmdResult<Vec<Country>> {
    state.stations.countries().await.map_err(directory_error)
}

/// «Tus emisoras», the newest first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn stations_liked(app: tauri::AppHandle) -> CmdResult<Vec<Station>> {
    with_library(&app, |l| l.liked_stations()).await
}

/// Saves `station` in «Tus emisoras» (as Radio Browser has it now, if it answers) or takes
/// it out.
#[tauri::command]
#[specta::specta]
pub(crate) async fn stations_like(app: tauri::AppHandle, station: Station, liked: bool) -> CmdResult<()> {
    let station = if liked { fresh(&app, station).await } else { station };
    with_library(&app, move |l| l.set_station_liked(&station, liked)).await
}

/// What `station_play` started: the session for `stream://localhost/_live/<session>`, and
/// the station as it plays (a favourite's stream may have moved).
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LivePlay {
    session: u32,
    station: Station,
}

/// Starts listening to `station` (stopping any other). Its «now playing» comes as
/// `live-title` events.
#[tauri::command]
#[specta::specta]
pub(crate) async fn station_play(app: tauri::AppHandle, station: Station) -> CmdResult<LivePlay> {
    let uuid = station.uuid.clone();
    let liked = with_library(&app, move |l| l.is_station_liked(&uuid)).await?;
    let station = if liked { fresh(&app, station).await } else { station };
    if liked {
        let saved = station.clone();
        with_library(&app, move |l| l.refresh_station(&saved)).await?;
    }
    let state = app.state::<AppState>();
    let sink: TitleSink = {
        let app = app.clone();
        Arc::new(move |title: LiveTitle| {
            let _ = app.emit(LIVE_TITLE_EVENT, title);
        })
    };
    let session = state.live.start(state.http.clone(), station.url.clone(), sink);
    // Radio Browser's popularity count: nobody waits for it.
    let uuid = station.uuid.clone();
    let counter = app.clone();
    tauri::async_runtime::spawn(async move { counter.state::<AppState>().stations.click(&uuid).await });
    Ok(LivePlay { session, station })
}

/// Stops the station (paused, or the player went back to songs).
#[tauri::command]
#[specta::specta]
pub(crate) fn station_stop(state: State<'_, AppState>) {
    state.live.stop();
}

/// Where Windows says this computer is, rounded to ~1 km. Fails if the user or Windows
/// doesn't allow it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn system_location() -> CmdResult<Coords> {
    tauri::async_runtime::spawn_blocking(location::system_position)
        .await
        .map_err(|e| format!("la tarea se interrumpió: {e}"))?
        .map_err(CmdError::from)
}

/// The country of the system's region setting ("ES"), if it has one.
#[tauri::command]
#[specta::specta]
pub(crate) fn system_country() -> Option<String> {
    location::system_country()
}

/// `station` as Radio Browser has it now; the one given if it doesn't answer (soon).
async fn fresh(app: &tauri::AppHandle, station: Station) -> Station {
    let state = app.state::<AppState>();
    match tokio::time::timeout(std::time::Duration::from_secs(4), state.stations.by_uuid(&station.uuid)).await {
        Ok(Ok(Some(now))) => now,
        _ => station,
    }
}

/// The directory failed: one message for the user, with the cause after it.
fn directory_error(e: anyhow::Error) -> CmdError {
    format!("No se pudo cargar la lista de emisoras. Comprueba tu conexión y vuelve a intentarlo. ({e:#})").into()
}
