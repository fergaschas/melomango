//! Icon in the system tray: a click opens Melomango; its menu plays, pauses and skips
//! without opening the window. With "close to tray" on, closing the window hides it and
//! the music keeps playing.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

/// Event sent to the page for the player buttons ("toggle", "prev", "next").
pub const EVENT: &str = "tray-action";
const TRAY_ID: &str = "main";
/// Windows cuts tooltips at 128 characters.
const TOOLTIP_MAX: usize = 120;

pub struct Tray {
    toggle: MenuItem<Wry>,
    /// Closing the window hides it instead of quitting (set from the page's settings).
    pub close_to_tray: AtomicBool,
}

impl Tray {
    pub fn create(app: &AppHandle) -> tauri::Result<Self> {
        let toggle = MenuItem::with_id(app, "toggle", "Reproducir", true, None::<&str>)?;
        let prev = MenuItem::with_id(app, "prev", "Anterior", true, None::<&str>)?;
        let next = MenuItem::with_id(app, "next", "Siguiente", true, None::<&str>)?;
        let show = MenuItem::with_id(app, "show", "Mostrar Melomango", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
        let separator = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(app, &[&toggle, &prev, &next, &separator, &show, &quit])?;

        let mut builder = TrayIconBuilder::with_id(TRAY_ID)
            .tooltip("Melomango")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id().as_ref() {
                "show" => show_main(app),
                "quit" => app.exit(0),
                action @ ("toggle" | "prev" | "next") => {
                    let _ = app.emit(EVENT, action);
                }
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event
                {
                    show_main(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        builder.build(app)?;

        Ok(Self { toggle, close_to_tray: AtomicBool::new(true) })
    }

    /// Shows what is playing in the icon's tooltip and the menu's play/pause label.
    pub fn update(&self, app: &AppHandle, title: Option<&str>, playing: bool) {
        let _ = self.toggle.set_text(if playing { "Pausa" } else { "Reproducir" });
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_tooltip(Some(tooltip(title, playing)));
        }
    }

    pub fn set_close_to_tray(&self, on: bool) {
        self.close_to_tray.store(on, Ordering::Relaxed);
    }

    pub fn closes_to_tray(&self) -> bool {
        self.close_to_tray.load(Ordering::Relaxed)
    }
}

fn tooltip(title: Option<&str>, playing: bool) -> String {
    let text = match title {
        Some(title) if playing => format!("{title} · Melomango"),
        Some(title) => format!("{title} (en pausa) · Melomango"),
        None => "Melomango".into(),
    };
    if text.chars().count() > TOOLTIP_MAX {
        let cut: String = text.chars().take(TOOLTIP_MAX - 1).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// Brings the main window back (from the tray, minimized or behind other windows).
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_names_the_song() {
        assert_eq!(tooltip(Some("Copenhague — Vetusta Morla"), true), "Copenhague — Vetusta Morla · Melomango");
        assert_eq!(tooltip(Some("Copenhague"), false), "Copenhague (en pausa) · Melomango");
        assert_eq!(tooltip(None, false), "Melomango");
        let long = tooltip(Some(&"x".repeat(300)), true);
        assert_eq!(long.chars().count(), TOOLTIP_MAX);
        assert!(long.ends_with('…'));
    }
}
