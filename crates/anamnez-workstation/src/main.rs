//! Tauri 2 shell — the native binary the clinic runs on a workstation. Hosts the
//! Leptos WASM in WKWebView (macOS) / WebView2 (Windows). The webview is purely a
//! view; all HTTP, secret storage, and `anamnez://` deep-link handling happen here.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod state;

use crate::state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("ANAMNEZ_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap_state,
            commands::enroll_from_uri,
            commands::login,
            commands::logout,
            commands::seal_session,
            commands::current_environment,
            commands::transport_health,
            commands::transport_enroll_exchange,
            commands::transport_login,
            commands::transport_refresh,
            commands::transport_logout,
            commands::transport_list_patients,
            commands::transport_get_patient_detail,
            commands::transport_search_codes,
            commands::transport_start_encounter,
            commands::transport_finish_encounter,
            commands::transport_create_observation,
            commands::transport_amend_observation,
            commands::transport_mark_observation_entered_in_error,
            commands::ui_list_patients,
            commands::ui_get_patient_detail,
            commands::ui_search_codes,
            commands::ui_start_encounter,
            commands::ui_finish_encounter,
            commands::ui_create_observation,
            commands::ui_amend_observation,
            commands::ui_mark_observation_entered_in_error,
        ])
        .run(tauri::generate_context!())
        .expect("anamnez-workstation: tauri runtime panicked");
}
