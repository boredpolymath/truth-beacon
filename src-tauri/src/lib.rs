pub mod circuit_breaker;
pub mod commands;
pub mod credentials;
pub mod detection;
pub mod gateway;
pub mod models;
pub mod storage;
pub mod vault;

use commands::*;

pub mod tray;
pub mod notification;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Register persistent system tray icon on startup (Phase 17.1)
            tray::setup_system_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Keep app persistent in system tray when user closes main window
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            list_benchmarks,
            create_benchmark,
            list_incidents,
            resolve_incident,
            reset_circuit_breaker,
            get_circuit_breaker_status,
            run_sandbox_simulation,
            eradicate_local_data,
            verify_bot_handshake,
            import_benchmarks_from_role,
            import_benchmark_by_snowflake,
            promote_incident_to_benchmark,
            get_taxonomy_tags,
            synchronize_benchmark_avatars,
            list_audit_logs,
            dispatch_desktop_notification,
            execute_notification_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running TruthBeacon application");
}
