// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use in_the_loop_lib::{commands, db, local_server, polling};
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Setup database
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("in-the-loop.db");

            let database = Arc::new(
                db::Database::new(db_path).expect("Failed to initialize database"),
            );

            let app_state = commands::AppState {
                db: database.clone(),
            };

            app.manage(app_state);

            // Start local server for CLI wrapper
            let db_clone = database.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = local_server::start_local_server(db_clone).await {
                    eprintln!("Failed to start local server: {}", e);
                }
            });

            // Start polling manager
            let polling_manager = polling::PollingManager::new(database.clone(), app.handle().clone());
            tauri::async_runtime::spawn(async move {
                polling_manager.start().await;
            });

            // Setup system tray
            setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::add_item,
            commands::get_items,
            commands::remove_item,
            commands::toggle_checked,
            commands::save_credentials,
            commands::save_settings,
            commands::get_settings,
            commands::save_setting,
            commands::get_setting,
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Create tray menu
    let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    // Load tray icon (using a simple generated icon for now)
    let icon_bytes = include_bytes!("../icons/icon.png");
    let icon = Image::from_bytes(icon_bytes)?;

    // Build tray icon
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("In The Loop")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }
        })
        .build(app)?;

    Ok(())
}
