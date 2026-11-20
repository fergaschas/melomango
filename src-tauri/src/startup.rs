//! When Melomango can't start normally because the library won't open: say so instead of
//! closing in silence and, if the library comes from a newer Melomango (another copy was
//! updated, an old one was reinstalled), update this copy right there.
//!
//! Without this, an old copy that finds a newer library dies before its interface (and
//! with it the automatic update check) ever loads, and stays stuck forever.

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

use crate::library::NewerLibrary;

const RELEASES: &str = "https://github.com/fergaschas/melomango/releases/latest";

pub fn library_failed(app: &AppHandle, error: anyhow::Error) {
    // The normal app can't run without its state: hide the window and stop its page.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        if let Ok(url) = "about:blank".parse() {
            let _ = window.navigate(url);
        }
    }
    let newer = error.downcast_ref::<NewerLibrary>().is_some();
    let app = app.clone();
    // Dialogs block: off the main thread.
    std::thread::spawn(move || {
        if newer {
            let version = app.package_info().version.to_string();
            let accepted = ask(
                &app,
                &format!(
                    "Tu biblioteca es de una versión de Melomango más nueva que esta ({version}), así que esta no puede \
                     abrirla.\n\nSe va a instalar la última versión. Tus canciones, playlists y Me gusta no se tocan."
                ),
                "Actualizar",
            );
            if accepted {
                match tauri::async_runtime::block_on(update(&app)) {
                    // The installer takes over from here (and closes this copy).
                    Ok(true) => {}
                    Ok(false) => tell(
                        &app,
                        &format!("Todavía no hay una versión más nueva publicada. Instala la última desde {RELEASES}"),
                    ),
                    Err(e) => tell(
                        &app,
                        &format!("No se pudo actualizar: {e:#}\n\nDescarga la última versión desde {RELEASES}"),
                    ),
                }
            }
        } else {
            tell(&app, &format!("No se pudo abrir tu biblioteca:\n\n{error:#}"));
        }
        app.exit(1);
    });
}

/// Installs the latest release if there is one. On Windows the installer closes the app.
async fn update(app: &AppHandle) -> anyhow::Result<bool> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(false);
    };
    update.download_and_install(|_, _| {}, || {}).await?;
    Ok(true)
}

fn ask(app: &AppHandle, text: &str, ok: &str) -> bool {
    app.dialog()
        .message(text)
        .title("Melomango")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(ok.into(), "Cerrar".into()))
        .blocking_show()
}

fn tell(app: &AppHandle, text: &str) {
    app.dialog()
        .message(text)
        .title("Melomango")
        .kind(MessageDialogKind::Error)
        .blocking_show();
}
