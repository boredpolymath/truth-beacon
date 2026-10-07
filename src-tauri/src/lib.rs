pub mod circuit_breaker;
pub mod commands;
pub mod credentials;
pub mod detection;
pub mod gateway;
pub mod models;
pub mod storage;
pub mod vault;

use commands::*;

pub mod notification;
pub mod tray;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Initialize local SQLite storage and run startup integrity verification (Phase 11.3 & 25.3)
            let storage = storage::StorageManager::default_instance()
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let _ = storage.verify_integrity();

            // Register persistent system tray icon on startup (Phase 17.1)
            tray::setup_system_tray(app)?;

            // If bot credentials are saved in OS Keychain, launch the Discord Gateway connection automatically
            if let Ok(guilds) = credentials::CredentialManager::list_registered_guilds() {
                if let Some(first_guild) = guilds.first() {
                    if let Ok(token) = credentials::CredentialManager::get_token(first_guild) {
                        tauri::async_runtime::spawn(async move {
                            gateway::daemon::start_global_daemon(&token).await;
                        });
                    }
                }
            }

            // Spawn background auto-updater check 5 seconds after startup
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                use tauri_plugin_updater::UpdaterExt;
                use tauri::Emitter;
                if let Ok(updater) = app_handle.updater() {
                    if let Ok(Some(update)) = updater.check().await {
                        log::info!("TruthBeacon update available: v{}", update.version);
                        let _ = app_handle.emit(
                            "truthbeacon://update-available",
                            serde_json::json!({
                                "version": update.version,
                                "body": update.body,
                                "date": update.date.map(|d| d.to_string()),
                            }),
                        );
                    }
                }
            });

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
            execute_notification_action,
            get_discord_config,
            save_discord_config,
            disconnect_discord,
            fetch_bot_guilds,
            check_for_updates,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running TruthBeacon application");
}
