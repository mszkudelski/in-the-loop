use crate::commands::AppState;
use crate::services::url_parser;
use crate::tray;
use crate::db::Item;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

pub const DEFAULT_SHORTCUT: &str = "Ctrl+Shift+Q";

#[cfg(target_os = "macos")]
mod accessibility {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation::boolean::CFBoolean;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: *const core_foundation::base::CFTypeRef) -> bool;
    }

    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub fn request_accessibility_permission() -> bool {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        unsafe {
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const _)
        }
    }
}

/// Check and request accessibility permission on macOS.
/// Returns true if already trusted.
#[cfg(target_os = "macos")]
pub fn ensure_accessibility() -> bool {
    if accessibility::is_trusted() {
        true
    } else {
        accessibility::request_accessibility_permission();
        false
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_accessibility() -> bool {
    true
}

pub fn register_shortcut(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    let shortcut: tauri_plugin_global_shortcut::Shortcut =
        shortcut_str.parse().map_err(|e| format!("Invalid shortcut: {:?}", e))?;

    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                handle_shortcut(&app_clone);
            }
        })
        .map_err(|e| format!("Failed to register shortcut: {}", e))?;

    Ok(())
}

pub fn get_shortcut_setting(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    state
        .db
        .get_setting("add_item_shortcut")
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_SHORTCUT.to_string())
}

pub fn handle_shortcut(app: &AppHandle) {
    let url = match app.clipboard().read_text() {
        Ok(text) => text.trim().to_string(),
        Err(e) => {
            let _ = app
                .notification()
                .builder()
                .title("In The Loop")
                .body(format!("Failed to read clipboard: {}", e))
                .show();
            return;
        }
    };

    if url.is_empty() {
        let _ = app
            .notification()
            .builder()
            .title("In The Loop")
            .body("Clipboard is empty")
            .show();
        return;
    }

    let parsed = match url_parser::parse_url(&url) {
        Ok(p) => p,
        Err(e) => {
            let _ = app
                .notification()
                .builder()
                .title("In The Loop")
                .body(format!("Not a valid URL: {}", e))
                .show();
            return;
        }
    };

    let item = Item {
        id: Uuid::new_v4().to_string(),
        item_type: parsed.item_type,
        title: parsed.suggested_title.clone(),
        url: Some(url),
        status: "waiting".to_string(),
        previous_status: None,
        metadata: serde_json::to_string(&parsed.metadata).unwrap_or_default(),
        last_checked_at: None,
        last_updated_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        archived_at: None,
        polling_interval_override: None,
        checked: false,
    };

    let state = app.state::<AppState>();
    match state.db.add_item(&item) {
        Ok(_) => {
            tray::refresh_tray(app, &state.db);
            let _ = app
                .notification()
                .builder()
                .title("In The Loop")
                .body(format!("Added: {}", parsed.suggested_title))
                .show();
        }
        Err(e) => {
            let _ = app
                .notification()
                .builder()
                .title("In The Loop")
                .body(format!("Failed to add item: {}", e))
                .show();
        }
    }
}

pub fn update_shortcut(app: &AppHandle, new_shortcut: &str) -> Result<(), String> {
    let shortcut: tauri_plugin_global_shortcut::Shortcut =
        new_shortcut.parse().map_err(|e| {
            format!("Invalid shortcut: {:?}", e)
        })?;

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;

    app.global_shortcut()
        .on_shortcut(shortcut, {
            let app = app.clone();
            move |_app_handle, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    handle_shortcut(&app);
                }
            }
        })
        .map_err(|e| format!("Failed to register shortcut: {}", e))?;

    let state = app.state::<AppState>();
    state
        .db
        .save_setting("add_item_shortcut", new_shortcut)
        .map_err(|e| e.to_string())?;

    Ok(())
}
