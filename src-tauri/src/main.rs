// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use in_the_loop_lib::{commands, db, local_server, polling, tray};
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tokio::sync::Mutex;

fn main() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init());

    // Register the updater plugin on desktop only
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            // Manage PendingUpdate state (desktop only)
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            app.manage(in_the_loop_lib::updater::PendingUpdate(Mutex::new(None)));

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
            let polling_manager =
                polling::PollingManager::new(database.clone(), app.handle().clone());
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
            commands::archive_closed_items,
            commands::archive_stale_items,
            commands::toggle_checked,
            commands::save_credentials,
            commands::save_settings,
            commands::get_settings,
            commands::save_setting,
            commands::get_setting,
            commands::open_url,
            commands::add_todo,
            commands::get_todos,
            commands::update_todo_status,
            commands::update_todo_date,
            commands::delete_todo,
            commands::bind_todo_to_item,
            commands::unbind_todo_from_item,
            commands::get_todo_ids_for_item,
            in_the_loop_lib::updater::fetch_update,
            in_the_loop_lib::updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
