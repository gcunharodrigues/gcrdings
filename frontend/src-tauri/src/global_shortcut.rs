//! A system-wide chord that starts and stops a recording.
//!
//! The tray already exposed toggle_recording, but reaching it meant finding the
//! menu bar. The point of this is the case where the meeting has already
//! started and the app is behind three other windows.

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Cmd+Alt+R on macOS, Ctrl+Alt+R elsewhere.
///
/// Cmd+Shift+R is deliberately not reused: that is the in-app pause chord, and
/// a global binding would shadow it inside every other application too.
pub fn default_shortcut() -> Shortcut {
    #[cfg(target_os = "macos")]
    let modifiers = Modifiers::SUPER | Modifiers::ALT;
    #[cfg(not(target_os = "macos"))]
    let modifiers = Modifiers::CONTROL | Modifiers::ALT;

    Shortcut::new(Some(modifiers), Code::KeyR)
}

pub fn shortcut_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "⌥⌘R"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "Ctrl+Alt+R"
    }
}

/// Registers the chord. A failure here is reported and then tolerated: another
/// application may already own it, and the app must still start.
pub fn register<R: Runtime>(app: &AppHandle<R>) {
    let shortcut = default_shortcut();

    if let Err(error) = app.global_shortcut().register(shortcut) {
        log::warn!(
            "Global recording shortcut {} could not be registered: {}. \
             Recording is still available from the tray and the window.",
            shortcut_label(),
            error
        );
    } else {
        log::info!(
            "Global recording shortcut registered as {}",
            shortcut_label()
        );
    }
}

/// Fires on key-down only. Without the state check the chord would toggle twice
/// per press — once down, once up — which reads as the recording never starting.
pub fn handle_event<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    if shortcut != &default_shortcut() {
        return;
    }

    log::info!("Global shortcut {} pressed", shortcut_label());
    crate::tray::toggle_recording_from_shortcut(app);
}
