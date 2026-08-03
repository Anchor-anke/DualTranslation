mod commands;
mod conversion;
mod credentials;
mod error;
mod platform;
mod privacy;
mod providers;
mod storage;
mod types;

use commands::AppState;
use platform::ShortcutRegistration;
use std::sync::Mutex;
use storage::Storage;
use tauri::Manager;
use tauri_plugin_sql::{Migration, MigrationKind};
use tauri_plugin_window_state::StateFlags;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = vec![Migration {
        version: 1,
        description: "initial schema",
        sql: include_str!("../migrations/001_initial.sql"),
        kind: MigrationKind::Up,
    }];
    let window_state_flags = StateFlags::POSITION | StateFlags::SIZE | StateFlags::MAXIMIZED;

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = platform::show_main_window(app);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags)
                .build(),
        )
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:dualtranslation.db", migrations)
                .build(),
        )
        .setup(|app| {
            let storage = tauri::async_runtime::block_on(Storage::initialize(app.handle()))?;
            let settings = tauri::async_runtime::block_on(storage.get_settings())?;
            let state = AppState::new(storage)?;
            app.manage(state);
            app.manage(ShortcutRegistration(Mutex::new(None)));
            platform::setup(app, &settings.shortcut)?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_always_on_top(settings.always_on_top)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::show_main_window,
            commands::hide_main_window,
            commands::register_global_shortcut,
            commands::unregister_global_shortcut,
            commands::read_clipboard_text,
            commands::write_clipboard_text,
            commands::scan_sensitive_text,
            commands::list_provider_profiles,
            commands::save_provider_profile,
            commands::test_provider_profile,
            commands::convert,
            commands::cancel_conversion,
            commands::adjust_conversion,
            commands::list_history,
            commands::get_history,
            commands::delete_history,
            commands::clear_history,
            commands::get_settings,
            commands::update_settings,
            commands::get_local_metrics,
            commands::clear_local_metrics,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running DualTranslation");
}
