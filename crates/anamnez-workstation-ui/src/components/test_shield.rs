//! Persistent red TEST ribbon. Shown whenever the daemon's `environment` is
//! `Test`. Sourced from `LoginResponse.environment` and `/v1/health`.

use anamnez_protocol::environment::Environment;
use leptos::prelude::*;

use crate::app::GlobalCtx;

#[component]
pub fn TestShield() -> impl IntoView {
    let ctx = expect_context::<GlobalCtx>();
    view! {
        {move || {
            if matches!(ctx.environment.get(), Environment::Test) {
                view! { <div class="banner test">"TEST ORTAMI — gerçek hasta verisi giremezsiniz"</div> }
                    .into_any()
            } else {
                view! {}.into_any()
            }
        }}
    }
}
