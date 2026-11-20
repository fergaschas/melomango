//! Mini player: the main window shrinks to a small always-on-top strip with the player,
//! and goes back to where (and how big) it was. The same page keeps playing, so nothing
//! is interrupted.

use std::sync::Mutex;

use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, Size, WebviewWindow};

const MINI: LogicalSize<f64> = LogicalSize { width: 420.0, height: 150.0 };
const MINI_MIN: LogicalSize<f64> = LogicalSize { width: 340.0, height: 140.0 };
/// The normal window's minimum (as in tauri.conf.json).
const NORMAL_MIN: LogicalSize<f64> = LogicalSize { width: 820.0, height: 560.0 };

/// How the window was before shrinking, to put it back.
#[derive(Default)]
pub struct MiniState(Mutex<Option<Saved>>);

#[derive(Clone, Copy)]
pub struct Saved {
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    maximized: bool,
}

impl MiniState {
    pub fn set(&self, window: &WebviewWindow, on: bool) -> tauri::Result<()> {
        let mut saved = self.0.lock().unwrap();
        if on {
            if saved.is_none() {
                *saved = Some(Saved {
                    size: window.outer_size()?,
                    position: window.outer_position()?,
                    maximized: window.is_maximized()?,
                });
            }
            window.unmaximize()?;
            window.set_min_size(Some(Size::Logical(MINI_MIN)))?;
            window.set_size(Size::Logical(MINI))?;
            window.set_always_on_top(true)?;
        } else {
            window.set_always_on_top(false)?;
            window.set_min_size(Some(Size::Logical(NORMAL_MIN)))?;
            if let Some(before) = saved.take() {
                window.set_size(Size::Physical(before.size))?;
                window.set_position(before.position)?;
                if before.maximized {
                    window.maximize()?;
                }
            } else {
                window.set_size(Size::Logical(LogicalSize { width: 1200.0, height: 780.0 }))?;
            }
        }
        Ok(())
    }
}
