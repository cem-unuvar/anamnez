//! Read-only patient detail. Calls `ui_get_patient_detail` (native-side, owns the
//! cert + access token).

use anamnez_protocol::error::ErrorEnvelope;
use anamnez_protocol::ids::PatientId;
use anamnez_protocol::patient::PatientDetail as P;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::tauri;

#[derive(Serialize)]
struct Args {
    #[serde(rename = "patientId")]
    patient_id: PatientId,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Reply {
    Ok(P),
    Err(ErrorEnvelope),
    Transport(String),
}

#[component]
pub fn PatientDetailView<F: Fn() + Send + 'static + Clone>(
    id: PatientId,
    on_back: F,
) -> impl IntoView {
    let detail: RwSignal<Option<P>> = RwSignal::new(None);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);

    let load = move || {
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match tauri::invoke::<Reply>("ui_get_patient_detail", Args { patient_id: id }).await {
                Ok(Reply::Ok(d)) => detail.set(Some(d)),
                Ok(Reply::Err(env)) => error.set(Some(format!("{env:?}"))),
                Ok(Reply::Transport(s)) => error.set(Some(s)),
                Err(e) => error.set(Some(e.to_string())),
            }
            loading.set(false);
        });
    };
    Effect::new(move |_| load());

    view! {
        <div>
            <button on:click=move |_| on_back()>"← Geri"</button>
            {move || match (loading.get(), error.get(), detail.get()) {
                (true, _, _) => view! { <p class="muted">"Yükleniyor…"</p> }.into_any(),
                (_, Some(e), _) => view! { <div class="error">{e}</div> }.into_any(),
                (_, _, Some(d)) => render_detail(d).into_any(),
                _ => view! {}.into_any(),
            }}
        </div>
    }
}

fn render_detail(d: P) -> impl IntoView {
    let dob = d.patient.date_of_birth.to_string();
    let full = format!("{} {}", d.patient.given_names, d.patient.family_name);
    view! {
        <h1>{full}</h1>
        <p class="muted">"Doğum tarihi: " {dob}</p>

        <section style="margin-top:24px;">
            <h2>"Aktif sorunlar"</h2>
            {if d.problem_list.is_empty() {
                view! { <p class="muted">"Liste boş."</p> }.into_any()
            } else {
                view! {
                    <ul>
                        {d.problem_list
                            .iter()
                            .map(|p| view! { <li>{p.display_text.clone()}</li> })
                            .collect::<Vec<_>>()}
                    </ul>
                }
                .into_any()
            }}
        </section>

        <section style="margin-top:24px;">
            <h2>"Alerjiler"</h2>
            {if d.allergies.is_empty() {
                view! { <p class="muted">"Bilinen alerji yok."</p> }.into_any()
            } else {
                view! {
                    <ul>
                        {d.allergies
                            .iter()
                            .map(|a| {
                                let line = format!(
                                    "{} ({})",
                                    a.display_text,
                                    severity_tr(a.severity),
                                );
                                view! { <li>{line}</li> }
                            })
                            .collect::<Vec<_>>()}
                    </ul>
                }
                .into_any()
            }}
        </section>

        <section style="margin-top:24px;">
            <h2>"İlaçlar"</h2>
            {if d.medications.is_empty() {
                view! { <p class="muted">"Aktif ilaç kaydı yok."</p> }.into_any()
            } else {
                view! {
                    <ul>
                        {d.medications
                            .iter()
                            .map(|m| view! { <li>{m.display_text.clone()}</li> })
                            .collect::<Vec<_>>()}
                    </ul>
                }
                .into_any()
            }}
        </section>

        <section style="margin-top:24px;">
            <h2>"Ziyaretler"</h2>
            {if d.encounters.is_empty() {
                view! { <p class="muted">"Ziyaret kaydı yok."</p> }.into_any()
            } else {
                view! {
                    <ul>
                        {d.encounters
                            .iter()
                            .map(|e| {
                                let line = format!(
                                    "{} — {}",
                                    e.started_at,
                                    e.reason_text,
                                );
                                view! { <li>{line}</li> }
                            })
                            .collect::<Vec<_>>()}
                    </ul>
                }
                .into_any()
            }}
        </section>
    }
}

fn severity_tr(s: anamnez_protocol::allergy::AllergySeverity) -> &'static str {
    use anamnez_protocol::allergy::AllergySeverity::*;
    match s {
        Mild => "hafif",
        Moderate => "orta",
        Severe => "şiddetli",
        LifeThreatening => "yaşamı tehdit eden",
    }
}
