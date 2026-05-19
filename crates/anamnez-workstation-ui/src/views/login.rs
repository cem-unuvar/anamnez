//! Login screen — email + password. On success, transitions to `AppMode::AppShell`
//! and caches the daemon's reported environment + idle-lock policy.
//!
//! When `locked = true`, the card prepends a "session was locked" notice so the
//! user understands why they're seeing the login form again. The fields and
//! submit path are identical either way.

use anamnez_client_core::AppMode;
use anamnez_protocol::auth::User;
use anamnez_protocol::environment::Environment;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::app::GlobalCtx;
use crate::tauri;

#[derive(Serialize)]
struct LoginArgs {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginEcho {
    user: User,
    environment: Environment,
    idle_lock_minutes: u32,
}

#[component]
pub fn Login(ctx: GlobalCtx, #[prop(optional)] locked: bool) -> impl IntoView {
    let email = RwSignal::new(
        ctx.user
            .get_untracked()
            .map(|u| u.email)
            .unwrap_or_default(),
    );
    let password = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    let on_submit = {
        let ctx = ctx.clone();
        move |_| {
            let email_v = email.get_untracked();
            let password_v = password.get_untracked();
            if email_v.is_empty() || password_v.is_empty() {
                error.set(Some("E-posta ve parola gereklidir".into()));
                return;
            }
            busy.set(true);
            error.set(None);
            ctx.mode.set(AppMode::LoggingIn);
            let ctx = ctx.clone();
            spawn_local(async move {
                let args = LoginArgs {
                    email: email_v,
                    password: password_v,
                };
                match tauri::invoke::<LoginEcho>("login", args).await {
                    Ok(echo) => {
                        ctx.user.set(Some(echo.user));
                        ctx.environment.set(echo.environment);
                        ctx.idle_lock_minutes.set(echo.idle_lock_minutes);
                        ctx.mode.set(AppMode::AppShell);
                        ctx.disconnected.set(false);
                    }
                    Err(e) => {
                        error.set(Some(format!("giriş başarısız: {e}")));
                        ctx.mode.set(if locked {
                            AppMode::Locked
                        } else {
                            AppMode::LoggedOut
                        });
                    }
                }
                busy.set(false);
            });
        }
    };

    let title = if locked { "Oturum kilitlendi" } else { "Giriş" };

    view! {
        <div class="center">
            <div class="card">
                <h1>{title}</h1>
                {move || if locked {
                    view! {
                        <p class="muted">
                            "Hareketsizlik nedeniyle oturumunuz sonlandırıldı. "
                            "Devam etmek için tekrar giriş yapın."
                        </p>
                    }
                    .into_any()
                } else {
                    view! {}.into_any()
                }}
                <label>
                    "E-posta"
                    <input
                        type="email"
                        prop:value=move || email.get()
                        on:input=move |ev| email.set(event_target_value(&ev))
                    />
                </label>
                <label>
                    "Parola"
                    <input
                        type="password"
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                        on:keydown={
                            let on_submit = on_submit.clone();
                            move |ev: leptos::ev::KeyboardEvent| {
                                if ev.key() == "Enter" {
                                    on_submit(());
                                }
                            }
                        }
                    />
                </label>
                {move || error.get().map(|e| view! { <div class="error">{e}</div> })}
                <div class="actions">
                    <button
                        class="primary"
                        prop:disabled=move || busy.get()
                        on:click={
                            let on_submit = on_submit.clone();
                            move |_| on_submit(())
                        }
                    >
                        {move || if busy.get() { "Giriş yapılıyor…" } else { "Giriş yap" }}
                    </button>
                </div>
            </div>
        </div>
    }
}
