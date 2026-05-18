//! Banner shown when the workstation can't reach the daemon. Driven by
//! `GlobalCtx::disconnected`, which the views toggle from transport errors.

use leptos::prelude::*;

use crate::app::GlobalCtx;

#[component]
pub fn ConnectionBanner() -> impl IntoView {
    let ctx = expect_context::<GlobalCtx>();
    view! {
        {move || {
            if ctx.disconnected.get() {
                view! { <div class="banner disconnected">"Klinik sunucusundan bağlantı kesildi"</div> }
                    .into_any()
            } else {
                view! {}.into_any()
            }
        }}
    }
}
