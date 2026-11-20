//! The window and integrations: mini player, tray icon, Discord.

use super::*;

/// Shrinks the main window to the mini player (always on top), or puts it back.
#[tauri::command]
pub(crate) fn set_mini_mode(app: tauri::AppHandle, mini: State<'_, mini::MiniState>, on: bool) -> CmdResult<()> {
    let window = app.get_webview_window("main").ok_or("no se encontró la ventana principal")?;
    mini.set(&window, on).map_err(|e| CmdError::from(e.to_string()))
}

/// What the tray icon shows: the song in its tooltip and play/pause in its menu.
#[tauri::command]
pub(crate) fn tray_update(app: tauri::AppHandle, title: Option<String>, playing: bool) {
    if let Some(tray) = app.try_state::<tray::Tray>() {
        tray.update(&app, title.as_deref(), playing);
    }
}

/// Whether closing the window hides it in the tray (and the music keeps playing).
#[tauri::command]
pub(crate) fn set_close_to_tray(app: tauri::AppHandle, on: bool) {
    if let Some(tray) = app.try_state::<tray::Tray>() {
        tray.set_close_to_tray(on);
    }
}

/// Shows what is playing in Discord ("Escuchando Melomango"), or clears it with `null`.
#[tauri::command]
pub(crate) fn discord_set(state: State<'_, AppState>, now: Option<discord::NowPlaying>) {
    state.discord.set(now);
}

#[tauri::command]
pub(crate) fn discord_status(state: State<'_, AppState>) -> discord::Status {
    state.discord.status()
}
