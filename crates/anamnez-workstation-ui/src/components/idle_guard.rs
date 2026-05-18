//! DOM-level activity tracking. Refreshes `last_activity` on `mousemove`, `keydown`,
//! `pointerdown`, `wheel`, `visibilitychange`. Every 30s it checks the elapsed time;
//! on threshold cross, transitions `AppMode::AppShell -> Locked` and calls
//! `seal_session` to drop the native-side access token.

use anamnez_client_core::AppMode;
use gloo_timers::callback::Interval;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

use crate::app::GlobalCtx;
use crate::tauri;

const CHECK_INTERVAL_MS: u32 = 30_000;

#[component]
pub fn IdleGuard() -> impl IntoView {
    let ctx = expect_context::<GlobalCtx>();
    let last_activity = RwSignal::new(tauri::now_ms());

    // Wire activity listeners on the document.
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        for event in ["mousemove", "keydown", "pointerdown", "wheel"] {
            let sig = last_activity;
            let cb = Closure::<dyn FnMut()>::new(move || {
                sig.set(tauri::now_ms());
            });
            let _ = doc
                .add_event_listener_with_callback(event, cb.as_ref().unchecked_ref());
            cb.forget();
        }
    }

    // Periodic check. `Interval::new` returns a handle we deliberately forget — the
    // guard lives for the whole app session.
    let ctx_for_tick = ctx.clone();
    let interval = Interval::new(CHECK_INTERVAL_MS, move || {
        let mode = ctx_for_tick.mode.get_untracked();
        if !matches!(mode, AppMode::AppShell) {
            return;
        }
        let now = tauri::now_ms();
        let timeout_ms = f64::from(ctx_for_tick.idle_lock_minutes.get_untracked()) * 60_000.0;
        let elapsed = now - last_activity.get_untracked();
        if elapsed >= timeout_ms {
            ctx_for_tick.mode.set(AppMode::Locked);
            spawn_local(async {
                let _ = tauri::invoke::<()>("seal_session", ()).await;
            });
        }
    });
    interval.forget();

    view! {}
}
