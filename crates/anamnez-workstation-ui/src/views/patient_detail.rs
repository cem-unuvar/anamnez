//! Patient detail page — full redesign around an in-progress clinical visit.
//!
//! Layout (top → bottom in one scroll):
//!
//!   PatientHeader            name · age · sex · back · start/finish visit
//!   SummaryChips             allergies · active problems · meds (compact)
//!   ActiveVisitPanel         visible only while an encounter is in progress
//!     VitalsStrip            BP / HR / Temp / SpO2 / RR / Wt / Ht as one row
//!     ObservationTimeline    cards for everything recorded this visit
//!     ObservationComposer    code-first add-observation form
//!     FinishVisitPanel       finish-encounter reason picker
//!   StartVisitPrompt         visible only when no encounter is open
//!   PastEncountersList       read-only history at the bottom
//!
//! Codes are required on every observation — the SPEC's "preliminary with
//! null code" escape hatch was removed. The composer makes the code picker
//! the primary input; clinicians fall back to `ANAMNEZ-SYM` when nothing
//! else fits.

use anamnez_protocol::codesystem::{CodeSystem, SearchHit, SearchResponse};
use anamnez_protocol::encounter::{
    Encounter, EncounterKind, EncounterStatus, FinishEncounterRequest, StartEncounterRequest,
};
use anamnez_protocol::error::ErrorEnvelope;
use anamnez_protocol::ids::{EncounterId, ObservationId, PatientId};
use anamnez_protocol::observation::{
    AmendObservationRequest, ManualObservationDraft, MarkEnteredInErrorRequest, Observation,
    ObservationPatch, ObservationStatus, ObservationValue,
};
use anamnez_protocol::patient::PatientDetail as P;
use anamnez_protocol::versioned::Versioned;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::tauri;

// ─── IPC envelope ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Reply<T> {
    Ok(T),
    Err(ErrorEnvelope),
    Transport(String),
}

fn reply_error<T>(r: Reply<T>) -> Option<String> {
    match r {
        Reply::Ok(_) => None,
        Reply::Err(env) => Some(format!("{env:?}")),
        Reply::Transport(s) => Some(s),
    }
}

// ─── Arg envelopes ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GetDetailArgs {
    #[serde(rename = "patientId")]
    patient_id: PatientId,
}
#[derive(Serialize)]
struct StartEncounterArgs {
    req: StartEncounterRequest,
}
#[derive(Serialize)]
struct FinishEncounterArgs {
    #[serde(rename = "encounterId")]
    encounter_id: EncounterId,
    req: FinishEncounterRequest,
}
#[derive(Serialize)]
struct CreateObservationArgs {
    draft: ManualObservationDraft,
}
#[derive(Serialize)]
struct AmendObservationArgs {
    #[serde(rename = "observationId")]
    observation_id: ObservationId,
    req: AmendObservationRequest,
}
#[derive(Serialize)]
struct MarkEnteredInErrorArgs {
    #[serde(rename = "observationId")]
    observation_id: ObservationId,
    req: MarkEnteredInErrorRequest,
}
#[derive(Serialize)]
struct SearchCodesArgs {
    system: CodeSystem,
    q: String,
    limit: Option<usize>,
}

// ─── Vitals strip — pre-mapped LOINC codes for the common bedside vitals ────

struct VitalSlot {
    label_tr: &'static str,
    loinc: &'static str,
    unit: &'static str,
}

/// 8 fixed slots. Order is fixed so the grid layout is stable across reloads.
/// `unit` is also the wire unit sent to the server.
const VITAL_SLOTS: &[VitalSlot] = &[
    VitalSlot { label_tr: "Sistolik", loinc: "8480-6", unit: "mmHg" },
    VitalSlot { label_tr: "Diyastolik", loinc: "8462-4", unit: "mmHg" },
    VitalSlot { label_tr: "Nabız", loinc: "8867-4", unit: "/min" },
    VitalSlot { label_tr: "Vücut sıcaklığı", loinc: "8310-5", unit: "Cel" },
    VitalSlot { label_tr: "SpO2", loinc: "59408-5", unit: "%" },
    VitalSlot { label_tr: "Solunum sayısı", loinc: "9279-1", unit: "/min" },
    VitalSlot { label_tr: "Kilo", loinc: "29463-7", unit: "kg" },
    VitalSlot { label_tr: "Boy", loinc: "8302-2", unit: "cm" },
];

// ─── Entry component ─────────────────────────────────────────────────────────

#[component]
pub fn PatientDetailView<F: Fn() + Clone + Send + Sync + 'static>(
    id: PatientId,
    on_back: F,
) -> impl IntoView {
    let detail: RwSignal<Option<P>> = RwSignal::new(None);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);

    // `loading` is true only on the first fetch; reloads after a save keep
    // the existing view visible and quietly refresh, otherwise the whole
    // pane flashes to "Yükleniyor…" and the visit card looks like it
    // vanished.
    let load = move || {
        let is_initial = detail.with_untracked(Option::is_none);
        if is_initial {
            loading.set(true);
        }
        error.set(None);
        spawn_local(async move {
            match tauri::invoke::<Reply<P>>(
                "ui_get_patient_detail",
                GetDetailArgs { patient_id: id },
            )
            .await
            {
                Ok(Reply::Ok(d)) => detail.set(Some(d)),
                Ok(Reply::Err(env)) => error.set(Some(format!("{env:?}"))),
                Ok(Reply::Transport(s)) => error.set(Some(s)),
                Err(e) => error.set(Some(e.to_string())),
            }
            if is_initial {
                loading.set(false);
            }
        });
    };
    Effect::new(move |_| load());

    view! {
        <div class="patient-page">
            {move || match (loading.get(), error.get(), detail.get()) {
                (true, _, _) => view! { <p class="muted">"Yükleniyor…"</p> }.into_any(),
                (_, Some(e), _) => {
                    let back = on_back.clone();
                    view! {
                        <div>
                            <button on:click=move |_| back()>"← Hastalar"</button>
                            <div class="error">{e}</div>
                        </div>
                    }
                    .into_any()
                }
                (_, _, Some(d)) => {
                    let back = on_back.clone();
                    let reload = load.clone();
                    render_page(d, id, back, reload).into_any()
                }
                _ => view! {}.into_any(),
            }}
        </div>
    }
}

// ─── Top-level page composition ──────────────────────────────────────────────

fn render_page(
    d: P,
    patient_id: PatientId,
    on_back: impl Fn() + Clone + Send + Sync + 'static,
    reload: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let active = d
        .encounters
        .iter()
        .find(|v| matches!(v.value.status, EncounterStatus::InProgress))
        .cloned();

    let past_encounters: Vec<Versioned<Encounter>> = d
        .encounters
        .iter()
        .filter(|v| !matches!(v.value.status, EncounterStatus::InProgress))
        .cloned()
        .collect();

    let active_observations = d.active_encounter_observations.clone();

    view! {
        <PatientHeader
            patient=d.patient.clone()
            active=active.clone()
            on_back=on_back.clone()
        />
        <SummaryChips
            allergies=d.allergies.clone()
            problem_list=d.problem_list.clone()
            medications=d.medications.clone()
        />

        {match active.clone() {
            Some(enc) => view! {
                <ActiveVisitPanel
                    patient_id=patient_id
                    encounter=enc
                    observations=active_observations
                    reload=reload.clone()
                />
            }
            .into_any(),
            None => view! {
                <StartVisitPrompt patient_id=patient_id reload=reload.clone() />
            }
            .into_any(),
        }}

        <PastEncountersList encounters=past_encounters />
    }
}

// ─── Patient header ──────────────────────────────────────────────────────────

#[component]
fn PatientHeader<F: Fn() + Clone + Send + Sync + 'static>(
    patient: anamnez_protocol::patient::Patient,
    active: Option<Versioned<Encounter>>,
    on_back: F,
) -> impl IntoView {
    let full_name = format!("{} {}", patient.given_names, patient.family_name);
    let dob = patient.date_of_birth.to_string();
    let sex = sex_tr(patient.sex_assigned_at_birth);
    let visit_pill = active.as_ref().map(|_| {
        view! { <span class="pill pill-on">"Ziyaret açık"</span> }
    });

    view! {
        <header class="patient-header">
            <button class="patient-header-back" on:click=move |_| on_back()>"← Hastalar"</button>
            <div class="patient-header-id">
                <h1>{full_name}</h1>
                <p class="muted">
                    {dob} " · " {sex}
                </p>
            </div>
            <div class="patient-header-actions">
                {visit_pill}
            </div>
        </header>
    }
}

// ─── Summary chips ───────────────────────────────────────────────────────────

#[component]
fn SummaryChips(
    allergies: Vec<anamnez_protocol::allergy::Allergy>,
    problem_list: Vec<Observation>,
    medications: Vec<anamnez_protocol::medication::Medication>,
) -> impl IntoView {
    let any = !allergies.is_empty() || !problem_list.is_empty() || !medications.is_empty();
    if !any {
        return view! {
            <section class="summary-chips empty">
                <p class="muted">"Bu hasta için kayıtlı alerji, sorun veya ilaç yok."</p>
            </section>
        }
        .into_any();
    }

    let allergy_row = (!allergies.is_empty()).then(|| {
        let chips: Vec<_> = allergies
            .iter()
            .map(|a| {
                let label = format!(
                    "{} · {}",
                    a.display_text,
                    severity_tr(a.severity)
                );
                view! { <span class="chip chip-allergy">{label}</span> }
            })
            .collect();
        view! {
            <div class="summary-row">
                <span class="summary-label">"⚠ Alerjiler"</span>
                <div class="summary-items">{chips}</div>
            </div>
        }
    });

    let problem_row = (!problem_list.is_empty()).then(|| {
        let chips: Vec<_> = problem_list
            .iter()
            .map(|p| {
                let label = p.display_text.clone();
                view! { <span class="chip chip-problem">{label}</span> }
            })
            .collect();
        view! {
            <div class="summary-row">
                <span class="summary-label">"⚕ Aktif sorunlar"</span>
                <div class="summary-items">{chips}</div>
            </div>
        }
    });

    let med_row = (!medications.is_empty()).then(|| {
        let chips: Vec<_> = medications
            .iter()
            .map(|m| {
                let label = m.display_text.clone();
                view! { <span class="chip chip-med">{label}</span> }
            })
            .collect();
        view! {
            <div class="summary-row">
                <span class="summary-label">"℞ İlaçlar"</span>
                <div class="summary-items">{chips}</div>
            </div>
        }
    });

    view! {
        <section class="summary-chips">
            {allergy_row}
            {problem_row}
            {med_row}
        </section>
    }
    .into_any()
}

// ─── Start visit prompt (no active encounter) ────────────────────────────────

#[component]
fn StartVisitPrompt<R: Fn() + Clone + Send + Sync + 'static>(
    patient_id: PatientId,
    reload: R,
) -> impl IntoView {
    let reason_text = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);

    let start = {
        let reload = reload.clone();
        move |_| {
            if busy.get() {
                return;
            }
            let text = reason_text.get();
            if text.trim().is_empty() {
                err.set(Some("Ziyaret nedenini giriniz.".into()));
                return;
            }
            busy.set(true);
            err.set(None);
            let reload = reload.clone();
            spawn_local(async move {
                let req = StartEncounterRequest {
                    patient_id,
                    kind: EncounterKind::InPerson,
                    reason_text: text,
                };
                let res = tauri::invoke::<Reply<Versioned<Encounter>>>(
                    "ui_start_encounter",
                    StartEncounterArgs { req },
                )
                .await;
                match res {
                    Ok(reply) => {
                        if let Some(msg) = reply_error(reply) {
                            err.set(Some(msg));
                        } else {
                            reason_text.set(String::new());
                            reload();
                        }
                    }
                    Err(e) => err.set(Some(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    view! {
        <section class="active-visit-panel idle">
            <h2 class="visit-heading">"Yeni ziyaret başlat"</h2>
            <p class="muted">
                "Kısa bir gerekçe yazıp \"Ziyareti başlat\"a basın. "
                "Ziyaret açıldığında ölçümler ve gözlemler bu hasta için kaydedilecek."
            </p>
            {move || err.get().map(|e| view! { <div class="error">{e}</div> })}
            <div class="start-visit-row">
                <input
                    type="text"
                    placeholder="Ziyaret nedeni — örn. hipertansiyon kontrolü, reçete yenileme"
                    prop:value=move || reason_text.get()
                    on:input=move |ev| reason_text.set(event_target_value(&ev))
                />
                <button class="primary" on:click=start disabled=move || busy.get()>
                    {move || if busy.get() { "Başlatılıyor…" } else { "Ziyareti başlat" }}
                </button>
            </div>
        </section>
    }
}

// ─── Active visit panel ──────────────────────────────────────────────────────

#[component]
fn ActiveVisitPanel<R: Fn() + Clone + Send + Sync + 'static>(
    patient_id: PatientId,
    encounter: Versioned<Encounter>,
    observations: Vec<Versioned<Observation>>,
    reload: R,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);

    let reason_text = encounter.value.reason_text.clone();
    let started_at = encounter.value.started_at.to_string();
    let enc_id = encounter.value.id;

    view! {
        <section class="active-visit-panel">
            <header class="visit-heading-row">
                <h2 class="visit-heading">"Aktif ziyaret"</h2>
                <p class="muted visit-meta">
                    <span class="pill pill-on">"Devam ediyor"</span>
                    " · " <strong>{reason_text}</strong>
                    " · başladı " {started_at}
                </p>
            </header>

            {move || err.get().map(|e| view! { <div class="error">{e}</div> })}

            <VitalsStrip
                patient_id=patient_id
                encounter_id=enc_id
                reload=reload.clone()
                shared_busy=busy
                shared_err=err
            />

            <ObservationTimeline
                observations=observations.clone()
                reload=reload.clone()
            />

            <ObservationComposer
                patient_id=patient_id
                encounter_id=enc_id
                reload=reload.clone()
                shared_busy=busy
                shared_err=err
            />

            <FinishVisitPanel
                encounter=encounter.clone()
                reload=reload.clone()
                shared_busy=busy
                shared_err=err
            />
        </section>
    }
}

// ─── Vitals strip ────────────────────────────────────────────────────────────

#[component]
fn VitalsStrip<R: Fn() + Clone + Send + Sync + 'static>(
    patient_id: PatientId,
    encounter_id: EncounterId,
    reload: R,
    shared_busy: RwSignal<bool>,
    shared_err: RwSignal<Option<String>>,
) -> impl IntoView {
    // One signal per slot. Vec<RwSignal> rather than RwSignal<Vec> so each
    // input rerenders independently as the clinician tabs through.
    let slots: Vec<RwSignal<String>> = VITAL_SLOTS
        .iter()
        .map(|_| RwSignal::new(String::new()))
        .collect();

    let filled_count = {
        let slots = slots.clone();
        Memo::new(move |_| {
            slots
                .iter()
                .filter(|s| !s.get().trim().is_empty())
                .count()
        })
    };

    let inputs: Vec<_> = slots
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            let slot = &VITAL_SLOTS[i];
            let label = slot.label_tr;
            let unit = slot.unit;
            let sig = *sig;
            view! {
                <label class="vital-slot">
                    <span class="vital-slot-label">{label}</span>
                    <span class="vital-slot-input-row">
                        <input
                            type="text"
                            inputmode="decimal"
                            prop:value=move || sig.get()
                            on:input=move |ev| sig.set(event_target_value(&ev))
                        />
                        <span class="vital-slot-unit muted">{unit}</span>
                    </span>
                </label>
            }
        })
        .collect();

    let save = {
        let slots = slots.clone();
        let reload = reload.clone();
        move |_| {
            if shared_busy.get() {
                return;
            }
            let filled: Vec<(usize, String)> = slots
                .iter()
                .enumerate()
                .filter_map(|(i, sig)| {
                    let v = sig.get();
                    let t = v.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some((i, t.to_owned()))
                    }
                })
                .collect();
            if filled.is_empty() {
                shared_err.set(Some(
                    "Kaydetmeden önce en az bir vitale değer girin.".into(),
                ));
                return;
            }
            // Validate numbers before any network call so we can fail loudly.
            let mut drafts: Vec<ManualObservationDraft> = Vec::with_capacity(filled.len());
            for (i, raw) in &filled {
                let parsed: f64 = match raw.replace(',', ".").parse() {
                    Ok(v) => v,
                    Err(_) => {
                        shared_err.set(Some(format!(
                            "{} değeri sayı olmalı (\"{raw}\" çözümlenemedi).",
                            VITAL_SLOTS[*i].label_tr,
                        )));
                        return;
                    }
                };
                drafts.push(ManualObservationDraft {
                    patient_id,
                    code: VITAL_SLOTS[*i].loinc.to_owned(),
                    code_system: CodeSystem::Loinc,
                    display_text: VITAL_SLOTS[*i].label_tr.to_owned(),
                    status: ObservationStatus::Final,
                    is_problem_list_item: false,
                    encounter_id: Some(encounter_id),
                    value_quantity: Some(parsed),
                    value_unit: Some(VITAL_SLOTS[*i].unit.to_owned()),
                    value_text: None,
                });
            }

            shared_busy.set(true);
            shared_err.set(None);
            let slots = slots.clone();
            let reload = reload.clone();
            let filled_for_clear: Vec<usize> = filled.iter().map(|(i, _)| *i).collect();
            spawn_local(async move {
                let mut last_err: Option<String> = None;
                for draft in drafts {
                    let res = tauri::invoke::<Reply<Versioned<Observation>>>(
                        "ui_create_observation",
                        CreateObservationArgs { draft },
                    )
                    .await;
                    match res {
                        Ok(reply) => {
                            if let Some(msg) = reply_error(reply) {
                                last_err = Some(msg);
                                break;
                            }
                        }
                        Err(e) => {
                            last_err = Some(e.to_string());
                            break;
                        }
                    }
                }
                if let Some(msg) = last_err {
                    shared_err.set(Some(msg));
                } else {
                    // Clear only the slots we successfully saved.
                    for i in filled_for_clear {
                        slots[i].set(String::new());
                    }
                    reload();
                }
                shared_busy.set(false);
            });
        }
    };

    view! {
        <section class="vitals-strip">
            <div class="vitals-strip-header">
                <h3>"Vitaller"</h3>
                <button
                    class="primary"
                    on:click=save
                    disabled=move || shared_busy.get() || filled_count.get() == 0
                >
                    {move || {
                        let n = filled_count.get();
                        if shared_busy.get() {
                            "Kaydediliyor…".to_string()
                        } else if n == 0 {
                            "Kaydet".to_string()
                        } else {
                            format!("Kaydet ({n})")
                        }
                    }}
                </button>
            </div>
            <div class="vitals-grid">{inputs}</div>
        </section>
    }
}

// ─── Observation timeline ────────────────────────────────────────────────────

#[component]
fn ObservationTimeline<R: Fn() + Clone + Send + Sync + 'static>(
    observations: Vec<Versioned<Observation>>,
    reload: R,
) -> impl IntoView {
    if observations.is_empty() {
        return view! {
            <section class="obs-timeline empty">
                <h3>"Bu ziyarette kayıt yok"</h3>
                <p class="muted">
                    "Üstteki vital şeridinden ölçüm girin veya aşağıdaki forma gözlem ekleyin."
                </p>
            </section>
        }
        .into_any();
    }

    let count = observations.len();
    let cards: Vec<_> = observations
        .into_iter()
        .map(|v| {
            let reload = reload.clone();
            view! {
                <ObservationCard observation=v reload=reload />
            }
        })
        .collect();

    view! {
        <section class="obs-timeline">
            <h3>{format!("Bu ziyarette ({count})")}</h3>
            <div class="obs-cards">{cards}</div>
        </section>
    }
    .into_any()
}

// ─── Observation card with inline edit + delete ──────────────────────────────

#[component]
fn ObservationCard<R: Fn() + Clone + Send + Sync + 'static>(
    observation: Versioned<Observation>,
    reload: R,
) -> impl IntoView {
    let editing = RwSignal::new(false);
    let confirming_delete = RwSignal::new(false);

    view! {
        {move || {
            if editing.get() {
                let obs = observation.clone();
                let reload = reload.clone();
                view! {
                    <ObservationCardEdit
                        observation=obs
                        editing=editing
                        reload=reload
                    />
                }
                .into_any()
            } else {
                let obs = observation.clone();
                let reload = reload.clone();
                view! {
                    <ObservationCardRead
                        observation=obs
                        editing=editing
                        confirming_delete=confirming_delete
                        reload=reload
                    />
                }
                .into_any()
            }
        }}
    }
}

#[component]
fn ObservationCardRead<R: Fn() + Clone + Send + Sync + 'static>(
    observation: Versioned<Observation>,
    editing: RwSignal<bool>,
    confirming_delete: RwSignal<bool>,
    reload: R,
) -> impl IntoView {
    let obs = observation.value.clone();
    let recorded = format_time_short(obs.recorded_at);
    let value_line = value_line_for(&obs);
    let code_line = code_line_for(&obs);
    let display_text = obs.display_text.clone();
    let obs_status = obs.status;
    let obs_id = obs.id;
    let expected_version = observation.version;

    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);

    let confirm_delete = {
        let reload = reload.clone();
        move |_| {
            if busy.get() {
                return;
            }
            busy.set(true);
            err.set(None);
            let reload = reload.clone();
            spawn_local(async move {
                let res = tauri::invoke::<Reply<Versioned<Observation>>>(
                    "ui_mark_observation_entered_in_error",
                    MarkEnteredInErrorArgs {
                        observation_id: obs_id,
                        req: MarkEnteredInErrorRequest { expected_version },
                    },
                )
                .await;
                match res {
                    Ok(reply) => {
                        if let Some(msg) = reply_error(reply) {
                            err.set(Some(msg));
                        } else {
                            confirming_delete.set(false);
                            reload();
                        }
                    }
                    Err(e) => err.set(Some(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    view! {
        <article class="obs-card">
            <div class="obs-card-head">
                <span class="obs-card-time muted">{recorded}</span>
                {status_pill_view(obs_status)}
                <div class="obs-card-actions">
                    <button
                        class="link"
                        on:click=move |_| editing.set(true)
                        disabled=move || busy.get()
                    >
                        "düzenle"
                    </button>
                    <button
                        class="link danger"
                        on:click=move |_| confirming_delete.set(true)
                        disabled=move || busy.get()
                    >
                        "sil"
                    </button>
                </div>
            </div>
            <div class="obs-card-body">
                <strong class="obs-card-text">{display_text}</strong>
                {value_line.map(|t| view! { <span class="obs-card-value">{t}</span> })}
                {code_line.map(|t| view! { <span class="obs-card-code muted">{t}</span> })}
            </div>
            {move || err.get().map(|e| view! { <div class="error">{e}</div> })}
            {move || {
                if confirming_delete.get() {
                    let confirm_delete = confirm_delete.clone();
                    view! {
                        <div class="obs-card-confirm">
                            <span>"Bu gözlemi hatalı olarak işaretle?"</span>
                            <button
                                class="danger"
                                on:click=confirm_delete
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "İşaretleniyor…" } else { "Evet, sil" }}
                            </button>
                            <button on:click=move |_| confirming_delete.set(false)>
                                "Vazgeç"
                            </button>
                        </div>
                    }
                    .into_any()
                } else {
                    view! {}.into_any()
                }
            }}
        </article>
    }
}

#[component]
fn ObservationCardEdit<R: Fn() + Clone + Send + Sync + 'static>(
    observation: Versioned<Observation>,
    editing: RwSignal<bool>,
    reload: R,
) -> impl IntoView {
    let obs = observation.value.clone();
    let obs_id = obs.id;
    let expected_version = observation.version;

    let display_text = RwSignal::new(obs.display_text.clone());
    let (initial_qty, initial_unit, initial_text) = match obs.value.clone() {
        Some(ObservationValue::Quantity(q)) => (Some(q.value.to_string()), Some(q.unit), None),
        Some(ObservationValue::String(s)) => (None, None, Some(s)),
        _ => (None, None, None),
    };
    let qty = RwSignal::new(initial_qty.unwrap_or_default());
    let unit = RwSignal::new(initial_unit.unwrap_or_default());
    let text_val = RwSignal::new(initial_text.unwrap_or_default());

    let busy = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);

    let save = {
        let reload = reload.clone();
        move |_| {
            if busy.get() {
                return;
            }
            let text = display_text.get();
            if text.trim().is_empty() {
                err.set(Some("Not boş olamaz.".into()));
                return;
            }
            let qty_raw = qty.get();
            let qty_raw_trim = qty_raw.trim();
            let unit_raw = unit.get();
            let text_raw = text_val.get();
            let text_raw_trim = text_raw.trim();

            // Build the value patch. Three legal shapes:
            //   1. Quantity {value, unit}      — qty + unit, no text
            //   2. String text                 — text only
            //   3. None                        — everything blank
            let value_patch: Option<Option<ObservationValue>> = if !qty_raw_trim.is_empty() {
                let parsed: f64 = match qty_raw_trim.replace(',', ".").parse() {
                    Ok(v) => v,
                    Err(_) => {
                        err.set(Some(format!(
                            "Değer sayı olmalı (\"{qty_raw_trim}\" çözümlenemedi).",
                        )));
                        return;
                    }
                };
                if unit_raw.trim().is_empty() {
                    err.set(Some(
                        "Sayısal değer girdiniz; ölçüm birimini de belirtin.".into(),
                    ));
                    return;
                }
                if !text_raw_trim.is_empty() {
                    err.set(Some(
                        "Hem sayısal değer hem metin değeri girilemez — birini seçin.".into(),
                    ));
                    return;
                }
                Some(Some(ObservationValue::Quantity(
                    anamnez_protocol::observation::ValueQuantity {
                        value: parsed,
                        unit: unit_raw.trim().to_owned(),
                    },
                )))
            } else if !text_raw_trim.is_empty() {
                Some(Some(ObservationValue::String(text_raw_trim.to_owned())))
            } else {
                Some(None)
            };

            let patch = ObservationPatch {
                display_text: Some(text),
                value: value_patch,
                ..Default::default()
            };

            busy.set(true);
            err.set(None);
            let reload = reload.clone();
            spawn_local(async move {
                let res = tauri::invoke::<Reply<Versioned<Observation>>>(
                    "ui_amend_observation",
                    AmendObservationArgs {
                        observation_id: obs_id,
                        req: AmendObservationRequest {
                            expected_version,
                            patch,
                        },
                    },
                )
                .await;
                match res {
                    Ok(reply) => {
                        if let Some(msg) = reply_error(reply) {
                            err.set(Some(msg));
                        } else {
                            editing.set(false);
                            reload();
                        }
                    }
                    Err(e) => err.set(Some(e.to_string())),
                }
                busy.set(false);
            });
        }
    };

    view! {
        <article class="obs-card obs-card-editing">
            <div class="obs-card-head">
                <span class="obs-card-time muted">"Düzenleniyor"</span>
            </div>
            <label class="obs-edit-field">
                <span>"Not"</span>
                <input
                    type="text"
                    prop:value=move || display_text.get()
                    on:input=move |ev| display_text.set(event_target_value(&ev))
                />
            </label>
            <div class="obs-edit-value-row">
                <label class="obs-edit-num">
                    <span>"Sayı"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        prop:value=move || qty.get()
                        on:input=move |ev| qty.set(event_target_value(&ev))
                    />
                </label>
                <label class="obs-edit-unit">
                    <span>"Birim"</span>
                    <input
                        type="text"
                        prop:value=move || unit.get()
                        on:input=move |ev| unit.set(event_target_value(&ev))
                    />
                </label>
                <label class="obs-edit-text">
                    <span>"…veya metin"</span>
                    <input
                        type="text"
                        prop:value=move || text_val.get()
                        on:input=move |ev| text_val.set(event_target_value(&ev))
                    />
                </label>
            </div>
            {move || err.get().map(|e| view! { <div class="error">{e}</div> })}
            <div class="obs-edit-actions">
                <button class="primary" on:click=save disabled=move || busy.get()>
                    {move || if busy.get() { "Kaydediliyor…" } else { "Kaydet" }}
                </button>
                <button on:click=move |_| editing.set(false)>"Vazgeç"</button>
            </div>
            <p class="muted">
                "Kaydedildiğinde durum 'düzeltildi' olarak işaretlenir ve denetim "
                "izine yazılır."
            </p>
        </article>
    }
}

// ─── Observation composer (non-vital observations) ───────────────────────────

#[component]
fn ObservationComposer<R: Fn() + Clone + Send + Sync + 'static>(
    patient_id: PatientId,
    encounter_id: EncounterId,
    reload: R,
    shared_busy: RwSignal<bool>,
    shared_err: RwSignal<Option<String>>,
) -> impl IntoView {
    let pick: RwSignal<Option<SearchHit>> = RwSignal::new(None);
    let display_text = RwSignal::new(String::new());
    let qty = RwSignal::new(String::new());
    let unit = RwSignal::new(String::new());
    let text_val = RwSignal::new(String::new());
    let mark_final = RwSignal::new(false);
    let is_problem = RwSignal::new(false);

    // When the clinician picks a code, default the note to its Turkish label
    // unless they've already typed something.
    Effect::new(move |_| {
        if let Some(h) = pick.get() {
            if display_text.get().trim().is_empty() {
                if let Some(s) = h.display_tr.clone().or(h.display_en.clone()) {
                    display_text.set(s);
                }
            }
        }
    });

    let save = {
        let reload = reload.clone();
        move |_| {
            if shared_busy.get() {
                return;
            }
            let Some(picked) = pick.get() else {
                shared_err.set(Some(
                    "Önce bir kod seçin (tanı / belirti / lab). Her gözlem kodlu kaydedilir."
                        .into(),
                ));
                return;
            };

            let text = display_text.get();
            let text_trim = text.trim();
            // If the user didn't type anything, fall back to the code's
            // Turkish label — display_text is NOT NULL on the wire.
            let display_text_out = if text_trim.is_empty() {
                picked
                    .display_tr
                    .clone()
                    .or_else(|| picked.display_en.clone())
                    .unwrap_or_else(|| picked.code.clone())
            } else {
                text_trim.to_owned()
            };

            let qty_raw = qty.get();
            let qty_raw_trim = qty_raw.trim();
            let unit_raw = unit.get();
            let text_raw = text_val.get();
            let text_raw_trim = text_raw.trim();

            let (value_quantity, value_unit, value_text) = if !qty_raw_trim.is_empty() {
                let parsed: f64 = match qty_raw_trim.replace(',', ".").parse() {
                    Ok(v) => v,
                    Err(_) => {
                        shared_err.set(Some(format!(
                            "Değer sayı olmalı (\"{qty_raw_trim}\" çözümlenemedi).",
                        )));
                        return;
                    }
                };
                if unit_raw.trim().is_empty() {
                    shared_err.set(Some(
                        "Sayısal değer girdiniz; ölçüm birimini de belirtin.".into(),
                    ));
                    return;
                }
                if !text_raw_trim.is_empty() {
                    shared_err.set(Some(
                        "Hem sayısal değer hem metin değeri girilemez — birini seçin.".into(),
                    ));
                    return;
                }
                (
                    Some(parsed),
                    Some(unit_raw.trim().to_owned()),
                    None,
                )
            } else if !text_raw_trim.is_empty() {
                (None, None, Some(text_raw_trim.to_owned()))
            } else {
                (None, None, None)
            };

            let draft = ManualObservationDraft {
                patient_id,
                code: picked.code.clone(),
                code_system: picked.code_system,
                display_text: display_text_out,
                status: if mark_final.get() {
                    ObservationStatus::Final
                } else {
                    ObservationStatus::Preliminary
                },
                is_problem_list_item: is_problem.get(),
                encounter_id: Some(encounter_id),
                value_quantity,
                value_unit,
                value_text,
            };

            shared_busy.set(true);
            shared_err.set(None);
            let reload = reload.clone();
            spawn_local(async move {
                let res = tauri::invoke::<Reply<Versioned<Observation>>>(
                    "ui_create_observation",
                    CreateObservationArgs { draft },
                )
                .await;
                match res {
                    Ok(reply) => {
                        if let Some(msg) = reply_error(reply) {
                            shared_err.set(Some(msg));
                        } else {
                            pick.set(None);
                            display_text.set(String::new());
                            qty.set(String::new());
                            unit.set(String::new());
                            text_val.set(String::new());
                            mark_final.set(false);
                            is_problem.set(false);
                            reload();
                        }
                    }
                    Err(e) => shared_err.set(Some(e.to_string())),
                }
                shared_busy.set(false);
            });
        }
    };

    view! {
        <section class="composer">
            <h3>"Gözlem ekle"</h3>
            <p class="muted">
                "Her gözlem için önce bir kod seçin (tanı, belirti, lab vb.). "
                "Karşılığı yoksa "
                <strong>"Belirti / şikayet"</strong>
                " (ANAMNEZ-SYM) listesini kullanın."
            </p>

            <div class="composer-code-row">
                <CodeAutocomplete
                    allowed_systems=observation_systems()
                    default_system=CodeSystem::Icd10Tm
                    placeholder="Kod ara — örn. kanama, LDL, metformin"
                    on_pick=move |hit| pick.set(Some(hit))
                />
            </div>
            {move || pick.get().map(|h| {
                let label = h.display_tr.clone().or(h.display_en.clone()).unwrap_or_default();
                view! {
                    <p class="composer-picked">
                        "✓ Seçili: " <strong>{label}</strong>
                        " (" {h.code} " · " {system_tr(h.code_system)} ")"
                    </p>
                }
            })}

            <label class="composer-note">
                <span>"Not (opsiyonel — kodun Türkçe etiketi varsayılan olarak alınır)"</span>
                <input
                    type="text"
                    placeholder="ör. baş ağrısı 3 gündür"
                    prop:value=move || display_text.get()
                    on:input=move |ev| display_text.set(event_target_value(&ev))
                />
            </label>

            <fieldset class="composer-value">
                <legend class="muted">"Ölçüm değeri (opsiyonel)"</legend>
                <div class="composer-value-row">
                    <label class="composer-value-num">
                        <span>"Sayı"</span>
                        <input
                            type="text"
                            inputmode="decimal"
                            placeholder="130"
                            prop:value=move || qty.get()
                            on:input=move |ev| qty.set(event_target_value(&ev))
                        />
                    </label>
                    <label class="composer-value-unit">
                        <span>"Birim"</span>
                        <input
                            type="text"
                            placeholder="mg/dL, mmHg, °C, ..."
                            prop:value=move || unit.get()
                            on:input=move |ev| unit.set(event_target_value(&ev))
                        />
                    </label>
                </div>
                <label class="composer-value-text">
                    <span class="muted">"…veya metin değeri (\"negatif\", \"normal\")"</span>
                    <input
                        type="text"
                        prop:value=move || text_val.get()
                        on:input=move |ev| text_val.set(event_target_value(&ev))
                    />
                </label>
            </fieldset>

            <div class="composer-flags">
                <label class="checkbox-row">
                    <input
                        type="checkbox"
                        prop:checked=move || is_problem.get()
                        on:change=move |ev| is_problem.set(event_target_checked(&ev))
                    />
                    <span>
                        "Aktif sorun listesine ekle "
                        <span class="muted">"(kronik tanı / devam eden şikayet)"</span>
                    </span>
                </label>
                <label class="checkbox-row">
                    <input
                        type="checkbox"
                        prop:checked=move || mark_final.get()
                        on:change=move |ev| mark_final.set(event_target_checked(&ev))
                    />
                    <span>
                        "Son halini aldı "
                        <span class="muted">"(düzeltmek için iz bırakır)"</span>
                    </span>
                </label>
            </div>

            <div class="composer-actions">
                <button class="primary" on:click=save disabled=move || shared_busy.get()>
                    {move || if shared_busy.get() { "Kaydediliyor…" } else { "Gözlemi kaydet" }}
                </button>
            </div>
        </section>
    }
}

// ─── Finish visit panel ──────────────────────────────────────────────────────

#[component]
fn FinishVisitPanel<R: Fn() + Clone + Send + Sync + 'static>(
    encounter: Versioned<Encounter>,
    reload: R,
    shared_busy: RwSignal<bool>,
    shared_err: RwSignal<Option<String>>,
) -> impl IntoView {
    let enc_id = encounter.value.id;
    let version = encounter.version;

    let pick: RwSignal<Option<SearchHit>> = RwSignal::new(None);

    let finish = {
        let reload = reload.clone();
        move |_| {
            if shared_busy.get() {
                return;
            }
            let Some(picked) = pick.get() else {
                shared_err.set(Some(
                    "Bitirmek için bir tanı/şikayet kodu seçiniz.".into(),
                ));
                return;
            };
            shared_busy.set(true);
            shared_err.set(None);
            let reload = reload.clone();
            spawn_local(async move {
                let req = FinishEncounterRequest {
                    expected_version: version,
                    reason_code: picked.code.clone(),
                    reason_code_system: picked.code_system,
                };
                let res = tauri::invoke::<Reply<Versioned<Encounter>>>(
                    "ui_finish_encounter",
                    FinishEncounterArgs {
                        encounter_id: enc_id,
                        req,
                    },
                )
                .await;
                match res {
                    Ok(reply) => {
                        if let Some(msg) = reply_error(reply) {
                            shared_err.set(Some(msg));
                        } else {
                            reload();
                        }
                    }
                    Err(e) => shared_err.set(Some(e.to_string())),
                }
                shared_busy.set(false);
            });
        }
    };

    view! {
        <section class="finish-visit">
            <h3>"Ziyareti bitir"</h3>
            <p class="muted">
                "Ziyareti kapatmadan önce bunu özetleyen bir tanı veya şikayet kodu seçin."
            </p>
            <CodeAutocomplete
                allowed_systems=encounter_reason_systems()
                default_system=CodeSystem::AnamnezSym
                placeholder="Tanı / şikayet ara — örn. hipertansiyon"
                on_pick=move |hit| pick.set(Some(hit))
            />
            {move || pick.get().map(|h| {
                let label = h.display_tr.clone().or(h.display_en.clone()).unwrap_or_default();
                view! {
                    <p class="composer-picked">
                        "✓ Seçili: " <strong>{label}</strong>
                        " (" {h.code} " · " {system_tr(h.code_system)} ")"
                    </p>
                }
            })}
            <div class="finish-visit-actions">
                <button on:click=finish disabled=move || shared_busy.get()>
                    {move || if shared_busy.get() { "Bitiriliyor…" } else { "Ziyareti bitir" }}
                </button>
            </div>
        </section>
    }
}

// ─── Past encounters ─────────────────────────────────────────────────────────

#[component]
fn PastEncountersList(encounters: Vec<Versioned<Encounter>>) -> impl IntoView {
    if encounters.is_empty() {
        return view! {
            <section class="past-visits empty">
                <h3>"Geçmiş ziyaretler"</h3>
                <p class="muted">"Geçmiş ziyaret kaydı yok."</p>
            </section>
        }
        .into_any();
    }
    let rows: Vec<_> = encounters
        .into_iter()
        .map(|v| {
            let e = v.value;
            let label = format!(
                "{} — {} · {}",
                e.started_at,
                e.reason_text,
                encounter_status_tr(e.status),
            );
            view! { <li>{label}</li> }
        })
        .collect();
    view! {
        <section class="past-visits">
            <h3>"Geçmiş ziyaretler"</h3>
            <ul>{rows}</ul>
        </section>
    }
    .into_any()
}

// ─── Code-system autocomplete (kept from original, minor restyle hooks) ──────

#[component]
fn CodeAutocomplete<F: Fn(SearchHit) + Clone + Send + Sync + 'static>(
    allowed_systems: Vec<CodeSystem>,
    default_system: CodeSystem,
    placeholder: &'static str,
    on_pick: F,
) -> impl IntoView {
    let system = RwSignal::new(default_system);
    let query = RwSignal::new(String::new());
    let hits: RwSignal<Vec<SearchHit>> = RwSignal::new(Vec::new());

    Effect::new(move |_| {
        let q = query.get();
        let sys = system.get();
        if q.trim().is_empty() {
            hits.set(Vec::new());
            return;
        }
        spawn_local(async move {
            let res = tauri::invoke::<Reply<SearchResponse>>(
                "ui_search_codes",
                SearchCodesArgs {
                    system: sys,
                    q,
                    limit: Some(10),
                },
            )
            .await;
            match res {
                Ok(Reply::Ok(r)) => hits.set(r.hits),
                _ => hits.set(Vec::new()),
            }
        });
    });

    view! {
        <div class="code-autocomplete">
            <div class="code-autocomplete-row">
                <select
                    on:change=move |ev| {
                        if let Some(parsed) = parse_system_tag(&event_target_value(&ev)) {
                            system.set(parsed);
                        }
                    }
                >
                    {allowed_systems
                        .iter()
                        .map(|s| {
                            let tag = system_tag(*s);
                            let selected = *s == default_system;
                            view! {
                                <option value=tag selected=selected>
                                    {system_tr(*s)}
                                </option>
                            }
                        })
                        .collect::<Vec<_>>()}
                </select>
                <input
                    type="text"
                    placeholder=placeholder
                    prop:value=move || query.get()
                    on:input=move |ev| query.set(event_target_value(&ev))
                />
            </div>
            <ul class="code-autocomplete-hits">
                {move || {
                    let on_pick = on_pick.clone();
                    hits.get()
                        .into_iter()
                        .map(|h| {
                            let label = h
                                .display_tr
                                .clone()
                                .or(h.display_en.clone())
                                .unwrap_or_default();
                            let display = format!("{} — {}", h.code, label);
                            let h_clone = h.clone();
                            let on_pick = on_pick.clone();
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        on:click=move |_| on_pick(h_clone.clone())
                                    >
                                        {display}
                                    </button>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>
        </div>
    }
}

// ─── View helpers ────────────────────────────────────────────────────────────

fn observation_systems() -> Vec<CodeSystem> {
    // SPEC §Data Modelling — observation-scoped subset (SKRS-VP is encounter-only).
    vec![
        CodeSystem::Icd10Tm,
        CodeSystem::AnamnezSym,
        CodeSystem::Loinc,
        CodeSystem::Sut,
        CodeSystem::Atc,
        CodeSystem::Titck,
    ]
}

fn encounter_reason_systems() -> Vec<CodeSystem> {
    // SPEC: encounter.reason_code_system ∈ {ICD10TM, ANAMNEZ-SYM, SKRS-VP}.
    vec![
        CodeSystem::AnamnezSym,
        CodeSystem::Icd10Tm,
        CodeSystem::SkrsVp,
    ]
}

fn system_tag(s: CodeSystem) -> &'static str {
    match s {
        CodeSystem::Atc => "ATC",
        CodeSystem::Titck => "TITCK",
        CodeSystem::Icd10Tm => "ICD10TM",
        CodeSystem::Loinc => "LOINC",
        CodeSystem::Sut => "SUT",
        CodeSystem::SkrsVp => "SKRS-VP",
        CodeSystem::AnamnezSym => "ANAMNEZ-SYM",
    }
}

fn parse_system_tag(s: &str) -> Option<CodeSystem> {
    Some(match s {
        "ATC" => CodeSystem::Atc,
        "TITCK" => CodeSystem::Titck,
        "ICD10TM" => CodeSystem::Icd10Tm,
        "LOINC" => CodeSystem::Loinc,
        "SUT" => CodeSystem::Sut,
        "SKRS-VP" => CodeSystem::SkrsVp,
        "ANAMNEZ-SYM" => CodeSystem::AnamnezSym,
        _ => return None,
    })
}

fn system_tr(s: CodeSystem) -> &'static str {
    match s {
        CodeSystem::AnamnezSym => "Belirti / şikayet",
        CodeSystem::Icd10Tm => "Tanı (ICD-10)",
        CodeSystem::Loinc => "Laboratuvar (LOINC)",
        CodeSystem::Sut => "İşlem (SUT)",
        CodeSystem::Atc => "İlaç — etken madde (ATC)",
        CodeSystem::Titck => "İlaç ürünü (TİTCK)",
        CodeSystem::SkrsVp => "Başvuru nedeni (SKRS)",
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

fn encounter_status_tr(s: EncounterStatus) -> &'static str {
    match s {
        EncounterStatus::InProgress => "devam ediyor",
        EncounterStatus::Finished => "bitti",
        EncounterStatus::Cancelled => "iptal",
    }
}

fn sex_tr(s: anamnez_protocol::patient::SexAssignedAtBirth) -> &'static str {
    use anamnez_protocol::patient::SexAssignedAtBirth::*;
    match s {
        Female => "K",
        Male => "E",
        Intersex => "İnterseks",
        Unknown => "Bilinmiyor",
    }
}

fn status_pill_view(s: ObservationStatus) -> impl IntoView {
    let (class, label) = match s {
        ObservationStatus::Preliminary => ("pill pill-prelim", "ÖN"),
        ObservationStatus::Final => ("pill pill-final", "KESİN"),
        ObservationStatus::Amended => ("pill pill-amended", "DÜZELTİLDİ"),
        ObservationStatus::EnteredInError => ("pill pill-erased", "HATALI"),
    };
    view! { <span class=class>{label}</span> }
}

fn value_line_for(obs: &Observation) -> Option<String> {
    obs.value.as_ref().map(|v| match v {
        ObservationValue::Quantity(q) => format!("{} {}", q.value, q.unit),
        ObservationValue::String(s) => s.clone(),
        ObservationValue::Codeable { code, .. } => code.clone(),
    })
}

fn code_line_for(obs: &Observation) -> Option<String> {
    let (Some(cs), Some(code)) = (obs.code_system, obs.code.as_deref()) else {
        return None;
    };
    Some(format!("{} · {}", system_tr(cs), code))
}

fn format_time_short(ts: jiff::Timestamp) -> String {
    // jiff on wasm32-unknown-unknown can't read /etc/localtime — calling
    // `TimeZone::system()` or `Zoned::now()` panics at runtime and freezes
    // the view render mid-flight. UTC is safe to construct without any
    // platform calls; we'll do local-zone display once we wire up a
    // JS-backed clock + tz source.
    let zoned = ts.to_zoned(jiff::tz::TimeZone::UTC);
    format!("{:02}:{:02} UTC", zoned.hour(), zoned.minute())
}
