//! Historically rendered a full-screen modal when `AppMode == Locked`. That modal
//! covered the underlying Login form and stranded the user with no way to type a
//! password. The Locked view now renders its own combined banner + form, so this
//! component is intentionally a no-op — kept as a placeholder so the import sites
//! in `app.rs` continue to compile, and so future work can re-introduce visual
//! flourishes (e.g., an avatar-only "you were locked out" splash) without
//! re-wiring the app shell.

use leptos::prelude::*;

#[component]
pub fn LockOverlay() -> impl IntoView {
    view! {}
}
