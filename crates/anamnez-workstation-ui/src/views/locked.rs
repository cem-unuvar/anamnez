//! When AppMode == Locked, we render the standard Login view *under* a lock overlay
//! (drawn by `components::LockOverlay`). The user re-logs in and clears the lock.
//! Kept as a standalone view in case the UX diverges later (e.g., showing only the
//! current user's avatar with a password field rather than full email + password).

use leptos::prelude::*;

use crate::app::GlobalCtx;
use crate::views::Login;

#[component]
pub fn Locked(ctx: GlobalCtx) -> impl IntoView {
    view! { <Login ctx=ctx /> }
}
