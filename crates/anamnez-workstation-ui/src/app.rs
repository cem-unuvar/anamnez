//! Top-level component. Owns the global signals (`Session`, `BootstrapState`,
//! `AppMode`, `Disconnected`) and routes the user into the correct view.

use anamnez_client_core::AppMode;
use anamnez_protocol::auth::User;
use anamnez_protocol::environment::Environment;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

use crate::components::{ConnectionBanner, IdleGuard, LockOverlay, TestShield};
use crate::tauri;
use crate::views;

#[derive(Debug, Clone, Default)]
pub struct GlobalCtx {
    pub mode: RwSignal<AppMode>,
    pub environment: RwSignal<Environment>,
    pub idle_lock_minutes: RwSignal<u32>,
    pub user: RwSignal<Option<User>>,
    pub disconnected: RwSignal<bool>,
    pub last_error: RwSignal<Option<String>>,
}

impl GlobalCtx {
    pub fn login_failed(&self, msg: impl Into<String>) {
        self.last_error.set(Some(msg.into()));
    }
}

#[derive(Debug, Deserialize)]
struct BootstrapReply {
    has_workstation_credential: bool,
    #[allow(dead_code)]
    has_refresh_token: bool,
    idle_lock_minutes_cache: u32,
}

#[component]
pub fn App() -> impl IntoView {
    let ctx = GlobalCtx::default();
    provide_context(ctx.clone());

    // Initial bootstrap probe: do we have a workstation credential? If yes, jump to
    // the login screen; otherwise show the enrollment paste screen.
    let ctx_for_bootstrap = ctx.clone();
    spawn_local(async move {
        match tauri::invoke::<BootstrapReply>("bootstrap_state", ()).await {
            Ok(b) => {
                ctx_for_bootstrap
                    .idle_lock_minutes
                    .set(b.idle_lock_minutes_cache);
                ctx_for_bootstrap.mode.set(if b.has_workstation_credential {
                    AppMode::LoggedOut
                } else {
                    AppMode::Bootstrap
                });
            }
            Err(e) => {
                ctx_for_bootstrap
                    .last_error
                    .set(Some(format!("önyükleme hatası: {e}")));
            }
        }
    });

    let mode_for_show = ctx.mode;
    let ctx_for_view = ctx.clone();
    view! {
        <div class="shell">
            <TestShield />
            <ConnectionBanner />
            <main>
                {move || {
                    let m = mode_for_show.get();
                    let ctx = ctx_for_view.clone();
                    match m {
                        AppMode::Bootstrap => view! { <views::Bootstrap ctx=ctx /> }.into_any(),
                        AppMode::LoggedOut | AppMode::LoggingIn => {
                            view! { <views::Login ctx=ctx /> }.into_any()
                        }
                        AppMode::AppShell => view! { <views::Home ctx=ctx /> }.into_any(),
                        AppMode::Locked => view! { <views::Locked ctx=ctx /> }.into_any(),
                    }
                }}
            </main>
        </div>
        <IdleGuard />
        <LockOverlay />
    }
}
