// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use in_the_loop_lib::{commands, db, local_server, polling, tray};
use std::sync::Arc;
use tauri::{Manager, WindowEvent};

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
            tray::setup_tray(app)?;

            if let Ok(settings) = database.get_all_settings() {
                if let Some(window) = app.get_webview_window("main") {
                    if let Ok(current_size) = window.outer_size() {
                        let new_size = tauri::PhysicalSize::new(
                            settings.screen_width as u32,
                            current_size.height,
                        );
                        let _ = window.set_size(new_size);
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::add_item,
            commands::get_items,
            commands::remove_item,
            commands::archive_item,
            commands::archive_items,
            commands::unarchive_item,
            commands::archive_stale_items,
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
