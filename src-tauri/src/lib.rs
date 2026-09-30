pub mod circuit_breaker;
pub mod commands;
pub mod credentials;
pub mod detection;
pub mod gateway;
pub mod models;
pub mod storage;
pub mod vault;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            list_benchmarks,
            create_benchmark,
            list_incidents,
            resolve_incident,
            reset_circuit_breaker,
            run_sandbox_simulation,
            eradicate_local_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running TruthBeacon application");
}
