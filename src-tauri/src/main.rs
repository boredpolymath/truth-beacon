// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    truth_beacon_lib::diagnostics::init_sanitized_logger();
    truth_beacon_lib::run();
}
