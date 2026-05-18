//! Post-login app shell. Holds the patient list as the default view and routes to
//! `PatientDetail` when a row is clicked. No clinical CRUD UI in this slice.

use anamnez_protocol::ids::PatientId;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::GlobalCtx;
use crate::tauri;
use crate::views::patient_detail::PatientDetailView;
use crate::views::patient_list::PatientListView;

#[derive(Debug, Clone)]
enum Route {
    List,
    Detail(PatientId),
}

#[component]
pub fn Home(ctx: GlobalCtx) -> impl IntoView {
    let route = RwSignal::new(Route::List);

    let user_display = {
        let ctx = ctx.clone();
        move || {
            ctx.user
                .get()
                .map(|u| u.display_name)
                .unwrap_or_else(|| "—".into())
        }
    };

    let on_logout = {
        let ctx = ctx.clone();
        move |_| {
            let ctx = ctx.clone();
            spawn_local(async move {
                let _ = tauri::invoke::<()>("logout", ()).await;
                ctx.user.set(None);
                ctx.mode.set(anamnez_client_core::AppMode::LoggedOut);
            });
        }
    };

    view! {
        <div>
            <header
                style="display:flex; align-items:center; justify-content:space-between; \
                       padding-bottom:16px; border-bottom:1px solid var(--border); margin-bottom:24px;"
            >
                <div>
                    <strong>"anamnez"</strong>
                    " · "
                    <span class="muted">{user_display}</span>
                </div>
                <button on:click=on_logout>"Çıkış"</button>
            </header>
            {move || match route.get() {
                Route::List => {
                    let r = route;
                    view! { <PatientListView on_select=move |id| r.set(Route::Detail(id)) /> }
                        .into_any()
                }
                Route::Detail(id) => {
                    let r = route;
                    view! { <PatientDetailView id=id on_back=move || r.set(Route::List) /> }
                        .into_any()
                }
            }}
        </div>
    }
}
