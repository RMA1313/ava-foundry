use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::server::AppState;

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(state)
}

async fn live() -> Json<StatusResponse> {
    Json(StatusResponse { status: "ok" })
}

async fn ready(State(state): State<AppState>) -> Json<StatusResponse> {
    let ready = state.database.is_ready().await;
    Json(StatusResponse {
        status: if ready { "ok" } else { "unavailable" },
    })
}

