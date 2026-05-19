//! Enrollment screen — paste the `anamnez://enroll?...` URI minted by the clinic
//! admin. On success the workstation has a device credential and the user is moved
//! to the login screen.

use anamnez_client_core::AppMode;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;

use crate::app::GlobalCtx;
use crate::tauri;

#[component]
pub fn Bootstrap(ctx: GlobalCtx) -> impl IntoView {
    let uri = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    let on_submit = {
        let ctx = ctx.clone();
        move |_| {
            let uri_val = uri.get_untracked();
            if uri_val.trim().is_empty() {
                error.set(Some("URI boş bırakılamaz".into()));
                return;
            }
            busy.set(true);
            error.set(None);
            let ctx = ctx.clone();
            spawn_local(async move {
                #[derive(Serialize)]
                struct Args {
                    uri: String,
                }
                #[derive(serde::Deserialize)]
                struct Reply {
                    #[serde(rename = "workstationId")]
                    _workstation_id: Option<String>,
                }
                let _ = match tauri::invoke::<Reply>(
                    "enroll_from_uri",
                    Args { uri: uri_val },
                )
                .await
                {
                    Ok(_r) => {
                        ctx.last_error.set(None);
                        ctx.mode.set(AppMode::LoggedOut);
                        Ok::<(), ()>(())
                    }
                    Err(e) => {
                        error.set(Some(format!("kayıt başarısız: {e}")));
                        Err(())
                    }
                };
                busy.set(false);
            });
        }
    };

    let ctx_for_view = ctx.clone();
    view! {
        <div class="center">
            <div class="card">
                <h1>"İş istasyonu kaydı"</h1>
                {move || ctx_for_view.last_error.get().map(|msg| {
                    view! { <div class="error">{msg}</div> }
                })}
                <p class="muted">
                    "Yöneticiniz size bir " <code>"anamnez://enroll?…"</code>
                    " bağlantısı verecek. Aşağıya yapıştırın."
                </p>
                <label>
                    "Kayıt URI'si"
                    <textarea
                        rows="3"
                        placeholder="anamnez://enroll?host=…&fingerprint=…&token=…"
                        prop:value=move || uri.get()
                        on:input=move |ev| uri.set(event_target_value(&ev))
                    ></textarea>
                </label>
                {move || error.get().map(|e| view! { <div class="error">{e}</div> })}
                <div class="actions">
                    <button class="primary" prop:disabled=move || busy.get() on:click=on_submit.clone()>
                        {move || if busy.get() { "Kaydediliyor…" } else { "Kaydet" }}
                    </button>
                </div>
            </div>
        </div>
    }
}
