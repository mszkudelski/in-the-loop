// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use in_the_loop_lib::{commands, db, local_server, polling, shortcut, tray};
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tokio::sync::Mutex;

fn main() {
    // Read shortcut setting from DB before building the app
    // so we can register it via the plugin builder (avoids deadlock)
    let initial_shortcut = {
        let app_dir = dirs::data_dir()
            .map(|d| d.join("com.intheloop.app"))
            .unwrap_or_default();
        let db_path = app_dir.join("in-the-loop.db");
        if db_path.exists() {
            db::Database::new(db_path)
                .ok()
                .and_then(|database| database.get_setting("add_item_shortcut").ok().flatten())
                .unwrap_or_else(|| shortcut::DEFAULT_SHORTCUT.to_string())
        } else {
            shortcut::DEFAULT_SHORTCUT.to_string()
        }
    };

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED
                        | tauri_plugin_window_state::StateFlags::FULLSCREEN,
                )
                .build(),
        );

    // Register the updater plugin on desktop only
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(move |app| {
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

            // Prompt for accessibility permission once (required for global shortcuts on macOS)
            if database.get_setting("accessibility_prompted").ok().flatten().is_none() {
                shortcut::ensure_accessibility();
                let _ = database.save_setting("accessibility_prompted", "true");
            }

            // Register global shortcut after a delay to ensure the event loop is running
            let app_handle = app.handle().clone();
            let shortcut_str = initial_shortcut.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if let Err(e) = shortcut::register_shortcut(&app_handle, &shortcut_str) {
                    eprintln!("Failed to register global shortcut: {}", e);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                use tauri_plugin_window_state::AppHandleExt;
                let _ = window.app_handle().save_window_state(
                    tauri_plugin_window_state::StateFlags::all(),
                );
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
            commands::get_add_item_shortcut,
            commands::update_add_item_shortcut,
            commands::get_github_token_source,
            in_the_loop_lib::updater::fetch_update,
            in_the_loop_lib::updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
