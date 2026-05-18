//! Leptos CSR shell. Mounted to body by `run()` (invoked by wasm-bindgen on
//! module init via Trunk's `data-trunk rel="rust"` magic in index.html).
//!
//! The whole frontend is one wasm module. The router maps mode signals onto views:
//!   AppMode::Bootstrap → enrollment URI paste
//!   AppMode::LoggedOut → login
//!   AppMode::AppShell  → patient list + detail
//!   AppMode::Locked    → re-login with overlay
//!
//! All clinical interactions go through Tauri commands; no `fetch` in WASM.

#![cfg_attr(not(debug_assertions), allow(dead_code))]

pub mod app;
pub mod components;
pub mod tauri;
pub mod views;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn run() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"anamnez-workstation-ui: wasm start".into());
    mount_to_body(|| leptos::view! { <app::App /> });
}
