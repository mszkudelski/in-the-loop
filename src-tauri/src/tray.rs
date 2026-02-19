use crate::db::{Database, Item};
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub const TRAY_ID: &str = "main-tray";

fn status_emoji(status: &str) -> &'static str {
    match status {
        "waiting" => "\u{23F3}",
        "in_progress" => "\u{1F504}",
        "updated" => "\u{1F514}",
        "approved" => "\u{1F44D}",
        "merged" => "\u{1F7E3}",
        "completed" => "\u{2705}",
        "failed" => "\u{274C}",
        "archived" => "\u{1F4E6}",
        _ => "\u{2753}",
    }
}

fn type_label(item_type: &str) -> &'static str {
    match item_type {
        "slack_thread" => "Slack",
        "github_action" => "Action",
        "github_pr" => "PR",
        "copilot_agent" => "Copilot",
        "cli_session" => "CLI",
        "opencode_session" => "OpenCode",
        _ => "Item",
    }
}

fn item_url(item: &Item) -> Option<String> {
    if item.item_type == "opencode_session" {
        let meta: serde_json::Value = serde_json::from_str(&item.metadata).ok()?;
        let base_url = meta["opencode_url"].as_str()?;
        let session_id = meta["session_id"].as_str()?;
        Some(format!("{}/session/{}", base_url, session_id))
    } else {
        item.url.clone()
    }
}

fn open_url_external(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

fn build_menu(
    app: &AppHandle,
    items: &[Item],
) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let menu = Menu::new(app)?;

    if items.is_empty() {
        let empty = MenuItem::with_id(app, "no-items", "No active items", false, None::<&str>)?;
        menu.append(&empty)?;
    } else {
        for item in items {
            let emoji = status_emoji(&item.status);
            let label = type_label(&item.item_type);
            let title = if item.title.len() > 40 {
                format!("{}...", &item.title[..37])
            } else {
                item.title.clone()
            };
            let menu_label = format!("{} [{}] {}", emoji, label, title);
            let menu_id = format!("item:{}", item.id);
            let has_url = item_url(item).is_some();
            let menu_item = MenuItem::with_id(app, menu_id, menu_label, has_url, None::<&str>)?;
            menu.append(&menu_item)?;
        }
    }

    let sep = PredefinedMenuItem::separator(app)?;
    menu.append(&sep)?;

    let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
    menu.append(&show)?;

    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    menu.append(&quit)?;

    Ok(menu)
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon_bytes = include_bytes!("../icons/tray-icon.png");
    let icon = Image::from_bytes(icon_bytes)?;

    let initial_menu = build_menu(&app.handle(), &[])?;

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&initial_menu)
        .show_menu_on_left_click(true)
        .tooltip("In The Loop")
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id.starts_with("item:") {
                let item_id = &id[5..];
                let state = app.state::<crate::commands::AppState>();
                if let Ok(items) = state.db.get_visible_items() {
                    if let Some(item) = items.iter().find(|i| i.id == item_id) {
                        if let Some(url) = item_url(item) {
                            open_url_external(&url);
                        }
                    }
                }
            } else {
                match id {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                }
            }
        })
        .on_tray_icon_event(|_tray, _event| {})
        .build(app)?;

    Ok(())
}

pub fn update_tray_badge(app_handle: &AppHandle, db: &Arc<Database>) {
    let count = db.count_actionable_items().unwrap_or(0);

    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        let title = if count > 0 {
            Some(count.to_string())
        } else {
            None
        };
        let _ = tray.set_title(title.as_deref());
    }
}

pub fn rebuild_tray_menu(app_handle: &AppHandle, db: &Arc<Database>) {
    let items = db.get_visible_items().unwrap_or_default();

    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app_handle, &items) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

pub fn refresh_tray(app_handle: &AppHandle, db: &Arc<Database>) {
    update_tray_badge(app_handle, db);
    rebuild_tray_menu(app_handle, db);
}
