//! Visual overlay drawn when the session is sealed by the idle-lock. Hands the user
//! back to the login screen via `AppMode::Locked` — actual re-login happens through
//! the standard `views::Login` flow rendered underneath (the overlay just visually
//! conveys "you were locked out").

use anamnez_client_core::AppMode;
use leptos::prelude::*;

use crate::app::GlobalCtx;

#[component]
pub fn LockOverlay() -> impl IntoView {
    let ctx = expect_context::<GlobalCtx>();
    view! {
        {move || {
            if matches!(ctx.mode.get(), AppMode::Locked) {
                view! {
                    <div class="lock-overlay">
                        <div class="card">
                            <h1>"Oturum kilitlendi"</h1>
                            <p class="muted">
                                "Hareketsizlik nedeniyle oturumunuz sonlandırıldı. "
                                "Devam etmek için tekrar giriş yapın."
                            </p>
                        </div>
                    </div>
                }
                .into_any()
            } else {
                view! {}.into_any()
            }
        }}
    }
}
