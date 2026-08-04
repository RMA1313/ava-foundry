use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{ingestion, server::AppState};

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
}

#[derive(Deserialize)]
struct ProjectListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Deserialize)]
struct ProjectCreateRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct ProjectUpdateRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct LocalFileRequest {
    path: String,
}

#[derive(Deserialize)]
struct FolderScanRequest {
    path: String,
}

#[derive(Deserialize)]
struct CsvImportRequest {
    path: String,
}

#[derive(Deserialize)]
struct UrlRegisterRequest {
    url: String,
}

#[derive(Serialize)]
struct ProjectResponse {
    id: String,
    name: String,
    description: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct ProjectsResponse {
    items: Vec<ProjectResponse>,
    limit: u32,
    offset: u32,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
struct RegistrationResponse {
    source_id: String,
    job_id: String,
}

fn error_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: ErrorDetail {
                code,
                message: message.into(),
            },
        }),
    )
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/:id", get(get_project).put(update_project))
        .route(
            "/projects/:id/sources/local-file",
            post(register_local_file),
        )
        .route("/projects/:id/sources/folder-scan", post(scan_folder))
        .route("/projects/:id/sources/csv-import", post(import_csv))
        .route("/projects/:id/sources/url", post(register_url))
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

async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<ProjectCreateRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), (StatusCode, Json<ErrorResponse>)> {
    let repository = state.database.projects();
    let project = repository
        .create(payload.name.as_str(), payload.description.as_deref())
        .await
        .map_err(|err| {
            error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
        })?;
    Ok((StatusCode::CREATED, Json(project.into())))
}

async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let id = parse_uuid(&id)?;
    let repository = state.database.projects();
    let project = repository.get(id).await.map_err(|err| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            err.to_string(),
        )
    })?;
    match project {
        Some(project) => Ok(Json(project.into())),
        None => Err(error_response(
            StatusCode::NOT_FOUND,
            "project_not_found",
            "project not found",
        )),
    }
}

async fn list_projects(
    State(state): State<AppState>,
    Query(query): Query<ProjectListQuery>,
) -> Result<Json<ProjectsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let repository = state.database.projects();
    let items = repository
        .list_paginated(limit as i64, offset as i64)
        .await
        .map_err(|err| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                err.to_string(),
            )
        })?;
    Ok(Json(ProjectsResponse {
        items: items.into_iter().map(Into::into).collect(),
        limit,
        offset,
    }))
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<ProjectUpdateRequest>,
) -> Result<Json<ProjectResponse>, (StatusCode, Json<ErrorResponse>)> {
    let id = parse_uuid(&id)?;
    let repository = state.database.projects();
    let project = repository
        .update(id, payload.name.as_str(), payload.description.as_deref())
        .await
        .map_err(|err| {
            error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
        })?;
    match project {
        Some(project) => Ok(Json(project.into())),
        None => Err(error_response(
            StatusCode::NOT_FOUND,
            "project_not_found",
            "project not found",
        )),
    }
}

async fn register_local_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<LocalFileRequest>,
) -> Result<Json<RegistrationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let result = ingestion::register_local_file(
        &state.database,
        project_id,
        std::path::Path::new(&payload.path),
    )
    .await
    .map_err(|err| error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string()))?;
    Ok(Json(RegistrationResponse {
        source_id: result.source_id.to_string(),
        job_id: result.job_id.to_string(),
    }))
}

async fn register_url(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UrlRegisterRequest>,
) -> Result<Json<RegistrationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let result = ingestion::register_public_url(&state.database, project_id, &payload.url)
        .await
        .map_err(|err| {
            error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
        })?;
    Ok(Json(RegistrationResponse {
        source_id: result.source_id.to_string(),
        job_id: result.job_id.to_string(),
    }))
}

async fn scan_folder(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<FolderScanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let (discovered, created, skipped) = ingestion::scan_folder(
        &state.database,
        project_id,
        std::path::Path::new(&payload.path),
    )
    .await
    .map_err(|err| error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string()))?;
    Ok(Json(serde_json::json!({
        "discovered": discovered,
        "created": created,
        "skipped": skipped,
    })))
}

async fn import_csv(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<CsvImportRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let summary = ingestion::import_csv_urls(
        &state.database,
        project_id,
        std::path::Path::new(&payload.path),
    )
    .await
    .map_err(|err| error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string()))?;
    Ok(Json(serde_json::json!({
        "imported": summary.imported,
        "duplicate": summary.duplicate,
        "invalid": summary.invalid,
        "failed": summary.failed,
    })))
}

impl From<crate::project::Project> for ProjectResponse {
    fn from(value: crate::project::Project) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            description: value.description,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

fn parse_uuid(value: &str) -> Result<uuid::Uuid, (StatusCode, Json<ErrorResponse>)> {
    value.parse().map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "project id must be a valid uuid",
        )
    })
}
