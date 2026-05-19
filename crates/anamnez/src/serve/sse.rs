//! GET /v1/events — SSE stream filtered by patient_access for the authenticated user.

use crate::serve::app_state::{AppState, AuthContext};
use anamnez_protocol::events::{ServerEvent, ServerEventPayload};
use axum::extract::{Extension, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub async fn events(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    let rx = state.events.subscribe();
    let db = state.db.clone();
    let stream = BroadcastStream::new(rx).filter_map(move |res| {
        let viewer = auth.user.id;
        let db = db.clone();
        let ev = res.ok()?;
        let visible = match &ev.payload {
            ServerEventPayload::ForcedLogout { .. } => true, // ForcedLogout is broadcast-wide
            ServerEventPayload::ObservationAmendedElsewhere { patient_id, .. }
            | ServerEventPayload::ObservationEnteredInError { patient_id, .. }
            | ServerEventPayload::PatientAccessChanged { patient_id, .. } => {
                let pid: anamnez_core::ids::PatientId = (*patient_id).into();
                anamnez_core::patient_access::level_for(&db, viewer, pid)
                    .ok()
                    .flatten()
                    .is_some()
            }
        };
        if !visible {
            return None;
        }
        let frame = serialize(&ev).ok()?;
        Some(Ok(frame))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn serialize(ev: &ServerEvent) -> serde_json::Result<Event> {
    let data = serde_json::to_string(ev)?;
    Ok(Event::default().id(ev.id.to_string()).data(data))
}
