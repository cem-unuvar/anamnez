//! When AppMode == Locked, we render the Login view with `locked=true`. That swaps
//! the card heading to "Oturum kilitlendi" and adds the explanatory subtext, but
//! keeps the same email + password inputs so the user can resume their session.

use leptos::prelude::*;

use crate::app::GlobalCtx;
use crate::views::Login;

#[component]
pub fn Locked(ctx: GlobalCtx) -> impl IntoView {
    view! { <Login ctx=ctx locked=true /> }
}
