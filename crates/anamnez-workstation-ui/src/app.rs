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
    #[serde(default)]
    daemon: Option<serde_json::Value>,
    #[serde(default)]
    config_path: Option<String>,
}

#[component]
pub fn App() -> impl IntoView {
    let ctx = GlobalCtx::default();
    provide_context(ctx.clone());

    // Initial bootstrap probe: do we have a workstation credential? If yes, jump to
    // the login screen; otherwise show the enrollment paste screen. If the probe
    // itself errors (e.g. OS keychain ACL denied), we stash the message in
    // `last_error` so the bootstrap view can surface it — silently stranding the
    // user on the enrollment paste form is the bug we're fixing here.
    let ctx_for_bootstrap = ctx.clone();
    spawn_local(async move {
        match tauri::invoke::<BootstrapReply>("bootstrap_state", ()).await {
            Ok(b) => {
                ctx_for_bootstrap
                    .idle_lock_minutes
                    .set(b.idle_lock_minutes_cache);
                if !b.has_workstation_credential && b.daemon.is_some() {
                    // Config has a daemon entry but the keychain doesn't have the
                    // cert/key. Almost always means the OS secret store lost the
                    // entry (dev rebuilds, ACL prompt denied, manual keychain
                    // delete). Surface it instead of silently asking the user to
                    // re-enroll.
                    ctx_for_bootstrap.last_error.set(Some(format!(
                        "İş istasyonu yapılandırması mevcut ({}) ancak \
                         OS anahtarlığında sertifika bulunamadı. \
                         Yöneticinizden yeni bir kayıt bağlantısı isteyin.",
                        b.config_path.as_deref().unwrap_or("config.toml"),
                    )));
                }
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
                ctx_for_bootstrap.mode.set(AppMode::Bootstrap);
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
