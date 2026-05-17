use crate::serve::app_state::{AppState, AuthContext};
use crate::serve::error::ApiError;
use crate::serve::routes::patients::parse_uuid;
use anamnez_core::ids::SourceDocumentId;
use anamnez_core::source_document;
use anamnez_protocol::source_document as p;
use anamnez_protocol::versioned::Versioned;
use axum::extract::{Extension, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/source_documents", post(create))
        .route("/v1/source_documents/:id", get(get_one))
}

async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(input): Json<p::NewSourceDocument>,
) -> std::result::Result<Json<Versioned<p::SourceDocument>>, ApiError> {
    let v = source_document::create(&state.db, auth.user_id(), input.into())?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}

async fn get_one(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id_str): Path<String>,
) -> std::result::Result<Json<Versioned<p::SourceDocument>>, ApiError> {
    let id = SourceDocumentId(parse_uuid(&id_str)?);
    let v = source_document::get(&state.db, auth.user_id(), id)?;
    Ok(Json(Versioned::new(v.value.into(), v.version)))
}
