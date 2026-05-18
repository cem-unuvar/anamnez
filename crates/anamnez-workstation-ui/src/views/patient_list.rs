//! Read-only patient list. Calls `ui_list_patients` (native-side, owns the
//! cert + access token) and renders the results.

use anamnez_protocol::error::ErrorEnvelope;
use anamnez_protocol::ids::PatientId;
use anamnez_protocol::patient::{PatientListItem, PatientListQuery, PatientListResponse};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::app::GlobalCtx;
use crate::tauri;

#[derive(Serialize)]
struct Args {
    query: PatientListQuery,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Reply {
    Ok(PatientListResponse),
    Err(ErrorEnvelope),
    Transport(String),
}

#[component]
pub fn PatientListView<F: Fn(PatientId) + Send + 'static + Clone>(on_select: F) -> impl IntoView {
    let ctx = expect_context::<GlobalCtx>();
    let items: RwSignal<Vec<PatientListItem>> = RwSignal::new(Vec::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);

    let load = {
        let ctx = ctx.clone();
        move || {
            loading.set(true);
            error.set(None);
            let ctx = ctx.clone();
            spawn_local(async move {
                match tauri::invoke::<Reply>(
                    "ui_list_patients",
                    Args {
                        query: PatientListQuery::default(),
                    },
                )
                .await
                {
                    Ok(Reply::Ok(resp)) => {
                        items.set(resp.items);
                        ctx.disconnected.set(false);
                    }
                    Ok(Reply::Err(env)) => {
                        error.set(Some(format!("{env:?}")));
                    }
                    Ok(Reply::Transport(s)) => {
                        ctx.disconnected.set(true);
                        error.set(Some(s));
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }
                loading.set(false);
            });
        }
    };
    let load_clone = load.clone();
    Effect::new(move |_| load_clone());

    view! {
        <h1>"Hastalar"</h1>
        {move || match (loading.get(), error.get()) {
            (true, _) => view! { <p class="muted">"Yükleniyor…"</p> }.into_any(),
            (_, Some(e)) => view! { <div class="error">{e}</div> }.into_any(),
            _ if items.with(Vec::is_empty) => view! {
                <p class="muted">"Henüz erişiminizde olan hasta yok."</p>
            }
            .into_any(),
            _ => {
                let on_select = on_select.clone();
                view! {
                    <table>
                        <thead>
                            <tr>
                                <th>"Ad Soyad"</th>
                                <th>"Doğum tarihi"</th>
                                <th>"MRN"</th>
                                <th>"Erişim"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || items.with(|rows| {
                                rows.iter()
                                    .cloned()
                                    .map(|p| {
                                        let on_select = on_select.clone();
                                        let id = p.id;
                                        let full = format!("{} {}", p.given_names, p.family_name);
                                        view! {
                                            <tr class="row-link" on:click=move |_| on_select(id)>
                                                <td>{full}</td>
                                                <td>{p.date_of_birth.to_string()}</td>
                                                <td>{p.mrn.unwrap_or_else(|| "—".into())}</td>
                                                <td class="muted">
                                                    {format!("{:?}", p.access_level).to_lowercase()}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })}
                        </tbody>
                    </table>
                }
                .into_any()
            }
        }}
    }
}
