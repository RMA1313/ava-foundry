use axum::{
    extract::{multipart::Multipart, Path, Query, State},
    http::{HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::path::{Path as FsPath, PathBuf};
use std::time::Instant;
use tracing::{info_span, Instrument};
use uuid::Uuid;

use crate::{
    audio_asset::AudioAssetListFilter,
    duplicate::DuplicateGroupDetails,
    ingestion,
    job::{JobListFilter, JobStatus, JobType},
    media_asset::{MediaAsset, MediaAssetListFilter},
    project::{ProjectListFilter, ProjectListSort, ProjectSortField, ProjectStatus, SortDirection},
    server::AppState,
    source::{SourceListFilter, SourceListSort, SourceSortField, SourceType},
    stage_run::StageRun,
};

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
}

#[derive(Deserialize)]
struct ProjectListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    status: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    direction: Option<String>,
}

#[derive(Deserialize)]
struct ProjectCreateRequest {
    name: String,
    description: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct ProjectUpdateRequest {
    name: String,
    description: Option<String>,
    status: Option<String>,
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

#[derive(Deserialize)]
struct SourceListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    source_type: Option<String>,
    ingestion_status: Option<String>,
    search: Option<String>,
    sort: Option<String>,
    direction: Option<String>,
}

#[derive(Deserialize)]
struct SourceUpdateRequest {
    source_config: Option<String>,
    import_metadata: Option<String>,
    ingestion_status: Option<String>,
    failure_reason: Option<String>,
    file_count: Option<i64>,
    byte_count: Option<i64>,
}

#[derive(Deserialize)]
struct JobListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    job_type: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct MediaAssetListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    ingestion_status: Option<String>,
    inspection_status: Option<String>,
    extraction_status: Option<String>,
    media_kind: Option<String>,
}

#[derive(Deserialize)]
struct AudioAssetListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    processing_status: Option<String>,
}

#[derive(Deserialize)]
struct RetryRequest {
    available_at: Option<String>,
}

#[derive(Serialize)]
struct ProjectResponse {
    id: String,
    name: String,
    description: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct ProjectsResponse {
    items: Vec<ProjectResponse>,
    limit: u32,
    offset: u32,
    total: i64,
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
struct UploadResponse {
    items: Vec<UploadItemResponse>,
    total_files: u32,
    total_byte_size: u64,
}

#[derive(Serialize)]
struct UploadItemResponse {
    source_id: String,
    job_id: String,
    original_filename: String,
    stored_path: String,
    byte_size: u64,
    sha256: String,
}

#[derive(Serialize)]
struct RegistrationResponse {
    source_id: String,
    job_id: String,
}

#[derive(Serialize)]
struct SourceResponse {
    id: String,
    project_id: String,
    source_type: String,
    original_url: Option<String>,
    source_config: Option<String>,
    failure_reason: Option<String>,
    import_metadata: Option<String>,
    file_count: i64,
    byte_count: i64,
    ingestion_status: String,
    progress: String,
    failure_summary: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct SourcesResponse {
    items: Vec<SourceResponse>,
    limit: u32,
    offset: u32,
}

#[derive(Serialize)]
struct JobResponse {
    id: String,
    project_id: String,
    source_id: Option<String>,
    job_type: String,
    status: String,
    priority: i32,
    attempts: i32,
    progress: String,
    failure_summary: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct JobsResponse {
    items: Vec<JobResponse>,
    limit: u32,
    offset: u32,
}

#[derive(Serialize)]
struct AudioAssetResponse {
    id: String,
    project_id: String,
    media_asset_id: String,
    source_id: Option<String>,
    processing_status: String,
    normalization_profile: Option<String>,
    validation_status: Option<String>,
    validation_error: Option<String>,
    validated_at: Option<String>,
    duration_seconds: f64,
    audio_format: String,
    codec: String,
    sample_rate_hz: i32,
    channels: i32,
    byte_size: i64,
    storage_path: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct MediaAssetResponse {
    id: String,
    project_id: String,
    source_id: String,
    media_kind: String,
    staged_path: String,
    original_filename: Option<String>,
    byte_size: i64,
    ingestion_status: String,
    inspection_status: Option<String>,
    extraction_status: Option<String>,
    extraction_progress: Option<String>,
    extraction_tmp_path: Option<String>,
    extracted_at: Option<String>,
    container_format: Option<String>,
    duration_seconds: Option<f64>,
    codecs: Option<String>,
    sample_rate_hz: Option<i32>,
    channels: Option<i32>,
    inspected_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct AudioAssetsResponse {
    items: Vec<AudioAssetResponse>,
    limit: u32,
    offset: u32,
}

#[derive(Serialize)]
struct MediaAssetsResponse {
    items: Vec<MediaAssetResponse>,
    limit: u32,
    offset: u32,
}

#[derive(Serialize)]
struct DatasetImportResponse {
    imported: u32,
    duplicate: u32,
    invalid: u32,
    failed: u32,
    row_failures: Vec<RowFailureResponse>,
}

#[derive(Serialize)]
struct RowFailureResponse {
    row: u32,
    reason: String,
}

#[derive(Serialize)]
struct StageRunResponse {
    id: String,
    project_id: String,
    media_asset_id: Option<String>,
    audio_asset_id: Option<String>,
    job_id: Option<String>,
    stage_name: String,
    status: String,
    queue_wait_ms: i64,
    wall_time_ms: i64,
    input_bytes: i64,
    output_bytes: i64,
    input_duration_seconds: f64,
    throughput: Option<f64>,
    failure_reason: Option<String>,
    cpu_percent: Option<f64>,
    memory_bytes: Option<i64>,
}

#[derive(Serialize)]
struct MetricsSummaryResponse {
    stage_runs: Vec<StageRunResponse>,
    deduplication: DeduplicationMetricsResponse,
}

#[derive(Serialize)]
struct DeduplicationMetricsResponse {
    throughput: Option<f64>,
    candidate_count: i64,
    match_rate: f64,
    duplicate_ratio: f64,
    unique_duration_seconds: f64,
    processing_failures: i64,
}

#[derive(Deserialize)]
struct DuplicateListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Serialize)]
struct DuplicateDecisionResponse {
    id: String,
    audio_asset_low_id: String,
    audio_asset_high_id: String,
    candidate_fingerprint_low_id: String,
    candidate_fingerprint_high_id: String,
    algorithm: String,
    algorithm_version: String,
    duration_delta_seconds: f64,
    similarity_score: f64,
    decision: String,
    confidence: String,
    duplicate_group_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct DuplicateGroupMemberResponse {
    duplicate_group_id: String,
    audio_asset_id: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct DuplicateGroupResponse {
    id: String,
    project_id: String,
    canonical_audio_asset_id: Option<String>,
    created_at: String,
    updated_at: String,
    members: Vec<DuplicateGroupMemberResponse>,
    decisions: Vec<DuplicateDecisionResponse>,
}

#[derive(Serialize)]
struct DuplicateGroupsResponse {
    items: Vec<DuplicateGroupResponse>,
    limit: u32,
    offset: u32,
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

fn database_error(err: anyhow::Error) -> (StatusCode, Json<ErrorResponse>) {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "database_error",
        err.to_string(),
    )
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/staging/capacity", get(staging_capacity))
        .route("/projects", get(list_projects).post(create_project))
        .route(
            "/projects/:id",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/projects/:id/sources", get(list_sources))
        .route(
            "/projects/:id/sources/:source_id",
            get(get_source).put(update_source).delete(delete_source),
        )
        .route("/projects/:id/sources/:source_id/retry", post(retry_source))
        .route(
            "/projects/:id/sources/:source_id/cancel",
            post(cancel_source),
        )
        .route("/projects/:id/jobs", get(list_jobs))
        .route("/projects/:id/media-assets", get(list_media_assets))
        .route("/projects/:id/audio-assets", get(list_audio_assets))
        .route(
            "/projects/:id/audio-assets/:audio_asset_id",
            get(get_audio_asset),
        )
        .route("/projects/:id/duplicates", get(list_duplicate_groups))
        .route("/projects/:id/duplicates/recover", post(recover_duplicates))
        .route("/projects/:id/metrics/summary", get(metrics_summary))
        .route("/projects/:id/jobs/recover", post(recover_jobs))
        .route("/projects/:id/pipeline/pause", post(pause_pipeline))
        .route("/projects/:id/pipeline/resume", post(resume_pipeline))
        .route("/projects/:project_id/jobs/:job_id/retry", post(retry_job))
        .route(
            "/projects/:id/sources/local-file",
            post(register_local_file),
        )
        .route("/projects/:id/sources/folder-scan", post(scan_folder))
        .route("/projects/:id/sources/csv-import", post(import_csv))
        .route("/projects/:id/sources/url", post(register_url))
        .route("/projects/:id/uploads/media", post(upload_media))
        .route("/projects/:id/uploads/archive", post(upload_archive))
        .route("/projects/:id/datasets/import", post(import_dataset))
        .layer(middleware::from_fn(request_logging_layer))
        .with_state(state)
}

async fn request_logging_layer(
    req: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let request_id = Uuid::new_v4();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let started = Instant::now();
    let span = info_span!(
        "http.request",
        request_id = %request_id,
        method = %method,
        path = %path,
        project_id = tracing::field::Empty,
        source_id = tracing::field::Empty,
        job_id = tracing::field::Empty,
        stage_id = tracing::field::Empty,
    );
    async move {
        let mut response = next.run(req).await;
        response.headers_mut().insert(
            "x-request-id",
            HeaderValue::from_str(&request_id.to_string()).unwrap(),
        );
        tracing::info!(
            request_id = %request_id,
            method = %method,
            path = %path,
            status = response.status().as_u16(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "http request completed"
        );
        response
    }
    .instrument(span)
    .await
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

async fn staging_capacity(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let capacity = state
        .staging_capacity_guard
        .check(&state.staging_dir)
        .map_err(database_error)?;
    Ok(Json(serde_json::json!({
        "paused": capacity.paused,
        "free_bytes": capacity.free_bytes,
        "total_bytes": capacity.total_bytes,
        "pause_free_bytes": capacity.pause_free_bytes,
        "resume_free_bytes": capacity.resume_free_bytes,
    })))
}

async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<ProjectCreateRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), (StatusCode, Json<ErrorResponse>)> {
    let repository = state.database.projects();
    let status = match payload.status {
        Some(value) => Some(ProjectStatus::parse(&value).map_err(|err| {
            error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
        })?),
        None => Some(ProjectStatus::Active),
    };
    let project = repository
        .create(payload.name.as_str(), payload.description.as_deref())
        .await
        .map_err(|err| {
            let status = if err.to_string() == "project name already exists" {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            let code = if status == StatusCode::CONFLICT {
                "conflict"
            } else {
                "invalid_request"
            };
            error_response(status, code, err.to_string())
        })?;
    let project = match status {
        Some(status) if status != ProjectStatus::Active => repository
            .update(
                project.id,
                &project.name,
                project.description.as_deref(),
                Some(status),
            )
            .await
            .map_err(|err| {
                error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
            })?
            .unwrap_or(project),
        _ => project,
    };
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
    let filter = ProjectListFilter {
        status: match query.status.as_deref() {
            Some(value) => Some(ProjectStatus::parse(value).map_err(|err| {
                error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
            })?),
            None => None,
        },
        search: query.search.clone(),
    };
    let sort = match query.sort.as_deref() {
        Some(value) => Some(ProjectListSort {
            field: ProjectSortField::parse(value).map_err(|err| {
                error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
            })?,
            direction: match query.direction.as_deref() {
                Some(value) => SortDirection::parse(value).map_err(|err| {
                    error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
                })?,
                None => SortDirection::Asc,
            },
        }),
        None => None,
    };
    let page = repository
        .list_paginated(Some(filter), sort, limit as i64, offset as i64)
        .await
        .map_err(|err| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                err.to_string(),
            )
        })?;
    Ok(Json(ProjectsResponse {
        items: page.items.into_iter().map(Into::into).collect(),
        limit,
        offset,
        total: page.total,
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
        .update(
            id,
            payload.name.as_str(),
            payload.description.as_deref(),
            match payload.status {
                Some(value) => Some(ProjectStatus::parse(&value).map_err(|err| {
                    error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
                })?),
                None => None,
            },
        )
        .await
        .map_err(|err| {
            let status = if err.to_string() == "project name already exists" {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            let code = if status == StatusCode::CONFLICT {
                "conflict"
            } else {
                "invalid_request"
            };
            error_response(status, code, err.to_string())
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

async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let id = parse_uuid(&id)?;
    let deleted = state
        .database
        .projects()
        .delete(id)
        .await
        .map_err(database_error)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(error_response(
            StatusCode::NOT_FOUND,
            "project_not_found",
            "project not found",
        ))
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

async fn list_sources(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<SourceListQuery>,
) -> Result<Json<SourcesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let source_type = match query.source_type.as_deref() {
        Some(value) => Some(parse_source_type(value)?),
        None => None,
    };
    let sort = match query.sort.as_deref() {
        Some(value) => Some(SourceListSort {
            field: SourceSortField::parse(value).map_err(|err| {
                error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
            })?,
            descending: matches!(query.direction.as_deref(), Some("desc")),
        }),
        None => None,
    };
    let page = state
        .database
        .sources()
        .list_paginated(
            project_id,
            Some(SourceListFilter {
                source_type,
                ingestion_status: query.ingestion_status.clone(),
                search: query.search.clone(),
            }),
            sort,
            query.limit.unwrap_or(50) as i64,
            query.offset.unwrap_or(0) as i64,
        )
        .await
        .map_err(database_error)?;
    Ok(Json(SourcesResponse {
        items: page.items.into_iter().map(source_response).collect(),
        limit: page.limit,
        offset: page.offset,
    }))
}

async fn get_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> Result<Json<SourceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let source_id = parse_uuid(&source_id)?;
    let source = state
        .database
        .sources()
        .get_in_project(project_id, source_id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            error_response(
                StatusCode::NOT_FOUND,
                "source_not_found",
                "source not found",
            )
        })?;
    Ok(Json(source_response(source)))
}

async fn update_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
    Json(payload): Json<SourceUpdateRequest>,
) -> Result<Json<SourceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let source_id = parse_uuid(&source_id)?;
    let mut source = state
        .database
        .sources()
        .get_in_project(project_id, source_id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            error_response(
                StatusCode::NOT_FOUND,
                "source_not_found",
                "source not found",
            )
        })?;
    if let Some(value) = payload.source_config.as_deref() {
        source = state
            .database
            .sources()
            .update_source_config(source_id, Some(value))
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    "source_not_found",
                    "source not found",
                )
            })?;
    }
    if let Some(value) = payload.import_metadata.as_deref() {
        source = state
            .database
            .sources()
            .update_import_metadata(source_id, Some(value))
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    "source_not_found",
                    "source not found",
                )
            })?;
    }
    if let Some(value) = payload.failure_reason.as_deref() {
        source = state
            .database
            .sources()
            .update_failure_state(source_id, Some(value))
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    "source_not_found",
                    "source not found",
                )
            })?;
    }
    if let Some(value) = payload.ingestion_status.as_deref() {
        source = state
            .database
            .sources()
            .update_ingestion_status(source_id, value)
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    "source_not_found",
                    "source not found",
                )
            })?;
    }
    if let (Some(file_count), Some(byte_count)) = (payload.file_count, payload.byte_count) {
        source = state
            .database
            .sources()
            .update_counters(source_id, file_count, byte_count)
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    "source_not_found",
                    "source not found",
                )
            })?;
    }
    Ok(Json(source_response(source)))
}

async fn delete_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let source_id = parse_uuid(&source_id)?;
    let deleted = state
        .database
        .sources()
        .delete(project_id, source_id)
        .await
        .map_err(database_error)?;
    if !deleted {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "source_not_found",
            "source not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn retry_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> Result<Json<SourceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let source_id = parse_uuid(&source_id)?;
    let source = state
        .database
        .sources()
        .retry(project_id, source_id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            error_response(
                StatusCode::NOT_FOUND,
                "source_not_found",
                "source not found",
            )
        })?;
    Ok(Json(source_response(source)))
}

async fn cancel_source(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
) -> Result<Json<SourceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let source_id = parse_uuid(&source_id)?;
    let source = state
        .database
        .sources()
        .cancel(project_id, source_id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            error_response(
                StatusCode::NOT_FOUND,
                "source_not_found",
                "source not found",
            )
        })?;
    Ok(Json(source_response(source)))
}

async fn list_jobs(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<JobListQuery>,
) -> Result<Json<JobsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let job_type = match query.job_type.as_deref() {
        Some(value) => Some(parse_job_type(value)?),
        None => None,
    };
    let status = match query.status.as_deref() {
        Some(value) => Some(parse_job_status(value)?),
        None => None,
    };
    let page = state
        .database
        .jobs()
        .list_paginated(
            Some(JobListFilter {
                project_id: Some(project_id),
                job_type,
                status,
            }),
            query.limit.unwrap_or(50) as i64,
            query.offset.unwrap_or(0) as i64,
        )
        .await
        .map_err(database_error)?;
    Ok(Json(JobsResponse {
        items: page.items.into_iter().map(job_response).collect(),
        limit: page.limit,
        offset: page.offset,
    }))
}

async fn list_media_assets(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<MediaAssetListQuery>,
) -> Result<Json<MediaAssetsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let page = state
        .database
        .media_assets()
        .list_paginated(
            MediaAssetListFilter {
                project_id,
                ingestion_status: query.ingestion_status.clone(),
                inspection_status: query.inspection_status.clone(),
                extraction_status: query.extraction_status.clone(),
                media_kind: query.media_kind.clone(),
            },
            query.limit.unwrap_or(50) as i64,
            query.offset.unwrap_or(0) as i64,
        )
        .await
        .map_err(database_error)?;
    Ok(Json(MediaAssetsResponse {
        items: page.items.into_iter().map(media_asset_response).collect(),
        limit: page.limit,
        offset: page.offset,
    }))
}

async fn list_audio_assets(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<AudioAssetListQuery>,
) -> Result<Json<AudioAssetsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let page = state
        .database
        .audio_assets()
        .list_paginated(
            AudioAssetListFilter {
                project_id,
                processing_status: query.processing_status.clone(),
            },
            query.limit.unwrap_or(50) as i64,
            query.offset.unwrap_or(0) as i64,
        )
        .await
        .map_err(database_error)?;
    let mut items = Vec::with_capacity(page.items.len());
    for audio_asset in page.items {
        let source_id = resolve_audio_asset_source_id(&state, audio_asset.media_asset_id).await?;
        items.push(audio_asset_response(audio_asset, source_id));
    }
    Ok(Json(AudioAssetsResponse {
        items,
        limit: page.limit,
        offset: page.offset,
    }))
}

async fn get_audio_asset(
    State(state): State<AppState>,
    Path((project_id, audio_asset_id)): Path<(String, String)>,
) -> Result<Json<AudioAssetResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let audio_asset_id = parse_uuid(&audio_asset_id)?;
    let audio_asset = state
        .database
        .audio_assets()
        .get_in_project(project_id, audio_asset_id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            error_response(
                StatusCode::NOT_FOUND,
                "audio_asset_not_found",
                "audio asset not found",
            )
        })?;
    let source_id = resolve_audio_asset_source_id(&state, audio_asset.media_asset_id).await?;
    Ok(Json(audio_asset_response(audio_asset, source_id)))
}

async fn metrics_summary(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MetricsSummaryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let id = Uuid::parse_str(&id).map_err(|err| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_project_id",
            err.to_string(),
        )
    })?;
    let runs = state
        .database
        .stage_runs()
        .list_by_project(id)
        .await
        .map_err(database_error)?;
    let throughput = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        select avg(throughput)
        from stage_runs
        where project_id = $1
          and stage_name = 'duplicate_detection'
          and status = 'completed'
          and throughput is not null
        "#,
    )
    .bind(id)
    .fetch_one(state.database.pool())
    .await
    .map_err(|err| database_error(err.into()))?;
    let candidate_count = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from audio_duplicate_decisions d
        join audio_assets a on a.id = d.audio_asset_low_id
        where a.project_id = $1
        "#,
    )
    .bind(id)
    .fetch_one(state.database.pool())
    .await
    .map_err(|err| database_error(err.into()))?;
    let duplicate_count = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from audio_duplicate_decisions d
        join audio_assets a on a.id = d.audio_asset_low_id
        where a.project_id = $1
          and d.decision = 'duplicate'
        "#,
    )
    .bind(id)
    .fetch_one(state.database.pool())
    .await
    .map_err(|err| database_error(err.into()))?;
    let unique_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        with project_assets as (
            select a.id, a.duration_seconds, m.duplicate_group_id, g.canonical_audio_asset_id
            from audio_assets a
            left join duplicate_group_memberships m on m.audio_asset_id = a.id
            left join duplicate_groups g on g.id = m.duplicate_group_id
            where a.project_id = $1
        )
        select coalesce(sum(
            case
                when duplicate_group_id is null then duration_seconds
                when canonical_audio_asset_id = id then duration_seconds
                else 0
            end
        ), 0)
        from project_assets
        "#,
    )
    .bind(id)
    .fetch_one(state.database.pool())
    .await
    .map_err(|err| database_error(err.into()))?
    .unwrap_or(0.0);
    let processing_failures = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from stage_runs
        where project_id = $1
          and stage_name = 'duplicate_detection'
          and status = 'failed'
        "#,
    )
    .bind(id)
    .fetch_one(state.database.pool())
    .await
    .map_err(|err| database_error(err.into()))?;
    let duplicate_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        with project_assets as (
            select a.id, a.duration_seconds, m.duplicate_group_id, g.canonical_audio_asset_id
            from audio_assets a
            left join duplicate_group_memberships m on m.audio_asset_id = a.id
            left join duplicate_groups g on g.id = m.duplicate_group_id
            where a.project_id = $1
        )
        select coalesce(sum(
            case
                when duplicate_group_id is null then 0
                when canonical_audio_asset_id = id then 0
                else duration_seconds
            end
        ), 0)
        from project_assets
        "#,
    )
    .bind(id)
    .fetch_one(state.database.pool())
    .await
    .map_err(|err| database_error(err.into()))?
    .unwrap_or(0.0);
    let total_duration_seconds = unique_duration_seconds + duplicate_duration_seconds;
    let match_rate = if candidate_count > 0 {
        duplicate_count as f64 / candidate_count as f64
    } else {
        0.0
    };
    let duplicate_ratio = if total_duration_seconds > 0.0 {
        duplicate_duration_seconds / total_duration_seconds
    } else {
        0.0
    };
    Ok(Json(MetricsSummaryResponse {
        stage_runs: runs.into_iter().map(stage_run_response).collect(),
        deduplication: DeduplicationMetricsResponse {
            throughput,
            candidate_count,
            match_rate,
            duplicate_ratio,
            unique_duration_seconds,
            processing_failures,
        },
    }))
}

async fn list_duplicate_groups(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<DuplicateListQuery>,
) -> Result<Json<DuplicateGroupsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let page = state
        .database
        .duplicates()
        .list_groups(
            project_id,
            query.limit.unwrap_or(50) as i64,
            query.offset.unwrap_or(0) as i64,
        )
        .await
        .map_err(database_error)?;
    Ok(Json(DuplicateGroupsResponse {
        items: page
            .items
            .into_iter()
            .map(duplicate_group_response)
            .collect(),
        limit: page.limit,
        offset: page.offset,
    }))
}

async fn recover_duplicates(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let _project_id = parse_uuid(&id)?;
    let recovered = state
        .database
        .duplicates()
        .recover_canonical_audio_assets()
        .await
        .map_err(database_error)?;
    Ok(Json(serde_json::json!({
        "recovered": recovered.len(),
        "items": recovered.into_iter().map(|(duplicate_group_id, canonical_audio_asset_id)| serde_json::json!({
            "duplicate_group_id": duplicate_group_id.to_string(),
            "canonical_audio_asset_id": canonical_audio_asset_id.to_string(),
        })).collect::<Vec<_>>(),
    })))
}

async fn recover_jobs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let recovered = state
        .database
        .jobs()
        .recover_expired_leases(chrono::Utc::now())
        .await
        .map_err(database_error)?
        .into_iter()
        .filter(|job| job.project_id == project_id)
        .count();
    Ok(Json(serde_json::json!({ "recovered": recovered })))
}

async fn pause_pipeline(
    State(state): State<AppState>,
    Path(_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state.staging_capacity_guard.pause();
    Ok(Json(serde_json::json!({ "paused": true })))
}

async fn resume_pipeline(
    State(state): State<AppState>,
    Path(_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    state.staging_capacity_guard.resume();
    Ok(Json(serde_json::json!({ "paused": false })))
}

fn stage_run_response(run: StageRun) -> StageRunResponse {
    StageRunResponse {
        id: run.id.to_string(),
        project_id: run.project_id.to_string(),
        media_asset_id: run.media_asset_id.map(|value| value.to_string()),
        audio_asset_id: run.audio_asset_id.map(|value| value.to_string()),
        job_id: run.job_id.map(|value| value.to_string()),
        stage_name: run.stage_name,
        status: run.status,
        queue_wait_ms: run.queue_wait_ms,
        wall_time_ms: run.wall_time_ms,
        input_bytes: run.input_bytes,
        output_bytes: run.output_bytes,
        input_duration_seconds: run.input_duration_seconds,
        throughput: run.throughput,
        failure_reason: run.failure_reason,
        cpu_percent: run.cpu_percent,
        memory_bytes: run.memory_bytes,
    }
}

fn duplicate_group_response(details: DuplicateGroupDetails) -> DuplicateGroupResponse {
    DuplicateGroupResponse {
        id: details.group.id.to_string(),
        project_id: details.group.project_id.to_string(),
        canonical_audio_asset_id: details
            .group
            .canonical_audio_asset_id
            .map(|id| id.to_string()),
        created_at: details.group.created_at.to_rfc3339(),
        updated_at: details.group.updated_at.to_rfc3339(),
        members: details
            .members
            .into_iter()
            .map(|member| DuplicateGroupMemberResponse {
                duplicate_group_id: member.duplicate_group_id.to_string(),
                audio_asset_id: member.audio_asset_id.to_string(),
                created_at: member.created_at.to_rfc3339(),
                updated_at: member.updated_at.to_rfc3339(),
            })
            .collect(),
        decisions: details
            .decisions
            .into_iter()
            .map(|decision| DuplicateDecisionResponse {
                id: decision.id.to_string(),
                audio_asset_low_id: decision.audio_asset_low_id.to_string(),
                audio_asset_high_id: decision.audio_asset_high_id.to_string(),
                candidate_fingerprint_low_id: decision.candidate_fingerprint_low_id.to_string(),
                candidate_fingerprint_high_id: decision.candidate_fingerprint_high_id.to_string(),
                algorithm: decision.algorithm,
                algorithm_version: decision.algorithm_version,
                duration_delta_seconds: decision.duration_delta_seconds,
                similarity_score: decision.similarity_score,
                decision: decision.decision,
                confidence: decision.confidence,
                duplicate_group_id: decision.duplicate_group_id.map(|id| id.to_string()),
                created_at: decision.created_at.to_rfc3339(),
                updated_at: decision.updated_at.to_rfc3339(),
            })
            .collect(),
    }
}

async fn retry_job(
    State(state): State<AppState>,
    Path((project_id, job_id)): Path<(String, String)>,
    Json(payload): Json<RetryRequest>,
) -> Result<Json<JobResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&project_id)?;
    let job_id = parse_uuid(&job_id)?;
    let job = state
        .database
        .jobs()
        .get(job_id)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "job_not_found", "job not found"))?;
    if job.project_id != project_id {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            "job_not_found",
            "job not found",
        ));
    }
    let available_at = payload
        .available_at
        .as_deref()
        .and_then(parse_rfc3339)
        .unwrap_or_else(chrono::Utc::now);
    let retried = state
        .database
        .jobs()
        .schedule_retry(job_id, "manual retry", available_at, 3)
        .await
        .map_err(database_error)?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "job_not_found", "job not found"))?;
    Ok(Json(job_response(retried)))
}

async fn scan_folder(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<FolderScanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let summary = ingestion::scan_folder(
        &state.database,
        project_id,
        std::path::Path::new(&payload.path),
    )
    .await
    .map_err(|err| error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string()))?;
    Ok(Json(serde_json::json!({
        "source_id": summary.source_id.to_string(),
        "discovered": summary.discovered,
        "created": summary.created,
        "changed": summary.changed,
        "removed": summary.removed,
        "skipped": summary.skipped,
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
        "row_failures": summary.row_failures.into_iter().map(|failure| serde_json::json!({
            "row": failure.row,
            "reason": failure.reason,
        })).collect::<Vec<_>>(),
    })))
}

async fn upload_file(
    state: AppState,
    id: String,
    mut multipart: Multipart,
    kind: &str,
) -> Result<Json<UploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let mut uploads = Vec::new();
    let mut file_count = 0u32;
    while let Some(field) = multipart.next_field().await.map_err(|err| {
        error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
    })? {
        if field.name().unwrap_or_default() == "file" {
            file_count += 1;
            if file_count > 20 {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "too many uploaded files",
                ));
            }
            let filename = field.file_name().unwrap_or(kind).to_string();
            let upload = persist_upload(&state, project_id, &filename, field, kind)
                .await
                .map_err(|err| {
                    error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
                })?;
            uploads.push(upload);
        }
    }
    if uploads.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "missing file upload",
        ));
    }
    let total_byte_size = uploads.iter().map(|upload| upload.byte_size).sum();
    Ok(Json(UploadResponse {
        total_files: uploads.len() as u32,
        total_byte_size,
        items: uploads
            .into_iter()
            .map(|upload| UploadItemResponse {
                source_id: upload.source_id.to_string(),
                job_id: upload.job_id.to_string(),
                original_filename: upload.original_filename,
                stored_path: upload.stored_path.display().to_string(),
                byte_size: upload.byte_size,
                sha256: upload.sha256,
            })
            .collect(),
    }))
}

struct PersistedUpload {
    source_id: Uuid,
    job_id: Uuid,
    original_filename: String,
    stored_path: PathBuf,
    byte_size: u64,
    sha256: String,
}

struct UploadConstraints {
    max_bytes: u64,
}

fn upload_constraints(kind: &str) -> UploadConstraints {
    let max_bytes = match kind {
        "archive" => 2_147_483_648,
        _ => 1_073_741_824,
    };
    UploadConstraints { max_bytes }
}

fn validate_upload_filename<'a>(filename: &'a str, kind: &str) -> anyhow::Result<&'a str> {
    let path = FsPath::new(filename);
    let basename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid filename"))?;
    if basename != filename
        || basename.is_empty()
        || basename == "."
        || basename == ".."
        || basename.starts_with('.')
    {
        return Err(anyhow::anyhow!("unsafe filename"));
    }
    let allowed: &[&str] = match kind {
        "media" => &[
            "wav", "mp3", "flac", "ogg", "m4a", "mp4", "mkv", "webm", "mov", "avi",
        ],
        "archive" => &["zip", "tar", "gz", "tgz"],
        _ => return Err(anyhow::anyhow!("unsupported upload kind")),
    };
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    if !extension
        .as_deref()
        .is_some_and(|ext| allowed.contains(&ext))
    {
        return Err(anyhow::anyhow!("invalid file extension"));
    }
    Ok(basename)
}

fn validate_upload_mime(
    content_type: Option<&str>,
    filename: &str,
    kind: &str,
) -> anyhow::Result<()> {
    let extension = FsPath::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let ok = match kind {
        "media" => matches!(
            (content_type.unwrap_or_default(), extension.as_str()),
            ("audio/wav", "wav")
                | ("audio/x-wav", "wav")
                | ("audio/mpeg", "mp3")
                | ("audio/flac", "flac")
                | ("audio/ogg", "ogg")
                | ("audio/mp4", "m4a")
                | ("video/mp4", "mp4")
                | ("video/x-matroska", "mkv")
                | ("video/webm", "webm")
                | ("video/quicktime", "mov")
                | ("video/x-msvideo", "avi")
        ),
        "archive" => matches!(
            (content_type.unwrap_or_default(), extension.as_str()),
            ("application/zip", "zip")
                | ("application/x-tar", "tar")
                | ("application/gzip", "gz")
                | ("application/x-gtar-compressed", "tgz")
                | ("application/x-tgz", "tgz")
        ),
        _ => false,
    };
    if !ok {
        return Err(anyhow::anyhow!("mime type does not match file extension"));
    }
    Ok(())
}

async fn persist_upload(
    state: &AppState,
    project_id: Uuid,
    filename: &str,
    mut field: axum::extract::multipart::Field<'_>,
    kind: &str,
) -> anyhow::Result<PersistedUpload> {
    let original_filename = validate_upload_filename(filename, kind)?.to_string();
    validate_upload_mime(field.content_type(), &original_filename, kind)?;
    let constraints = upload_constraints(kind);
    let upload_dir = state
        .staging_dir
        .join("uploads")
        .join(project_id.to_string());
    tokio::fs::create_dir_all(&upload_dir).await?;
    let upload_id = Uuid::new_v4();
    let temp_path = upload_dir.join(format!("{upload_id}.part"));
    let stored_path = upload_dir.join(format!("{upload_id}-{original_filename}"));
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut hasher = Sha256::new();
    let mut byte_size = 0u64;
    while let Some(chunk) = field.chunk().await? {
        byte_size = byte_size
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("uploaded file is too large"))?;
        if byte_size > constraints.max_bytes {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(anyhow::anyhow!("uploaded file exceeds size limit"));
        }
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    tokio::fs::rename(&temp_path, &stored_path).await?;
    let sha256 = format!("{:x}", hasher.finalize());

    let source_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let upload_metadata = format!(
        r#"{{"upload_kind":"{}","sha256":"{}","original_filename":"{}","stored_path":"{}"}}"#,
        kind,
        sha256,
        original_filename,
        stored_path.display()
    );
    let mut tx = state.database.pool().begin().await?;
    let source_row = sqlx::query(
        r#"
        insert into sources (
            id, project_id, source_type, original_url, normalized_identity,
            source_config, failure_reason, import_metadata, file_count, byte_count,
            ingestion_status, created_at, updated_at
        )
        values ($1, $2, 'local_file', $3, $4, $5, null, $6, 1, $7, 'pending', now(), now())
        on conflict (project_id, normalized_identity)
        do update set updated_at = excluded.updated_at
        returning id
        "#,
    )
    .bind(source_id)
    .bind(project_id)
    .bind(stored_path.to_string_lossy().to_string())
    .bind(format!(
        "local_file:{}",
        stored_path.to_string_lossy().to_lowercase()
    ))
    .bind(Some(upload_metadata.clone()))
    .bind(Some(upload_metadata))
    .bind(byte_size as i64)
    .fetch_one(&mut *tx)
    .await;
    let source_id = match source_row {
        Ok(row) => row.try_get::<Uuid, _>("id")?,
        Err(err) => {
            let _ = tokio::fs::remove_file(&stored_path).await;
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(err.into());
        }
    };
    if let Err(err) = sqlx::query(
        r#"
        insert into jobs (
            id, project_id, source_id, job_type, status,
            priority, attempts, available_at, lease_expires_at,
            worker_id, failure_reason, created_at, updated_at
        )
        values ($1, $2, $3, 'media_ingestion', 'queued',
                0, 0, now(), null, null, null, now(), now())
        on conflict (project_id, source_id, job_type)
        do update set updated_at = excluded.updated_at
        "#,
    )
    .bind(job_id)
    .bind(project_id)
    .bind(source_id)
    .execute(&mut *tx)
    .await
    {
        let _ = tokio::fs::remove_file(&stored_path).await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(err.into());
    }
    if let Err(err) = tx.commit().await {
        let _ = tokio::fs::remove_file(&stored_path).await;
        return Err(err.into());
    }

    Ok(PersistedUpload {
        source_id,
        job_id,
        original_filename,
        stored_path,
        byte_size,
        sha256,
    })
}

async fn persist_staged_upload(
    state: &AppState,
    project_id: Uuid,
    filename: String,
    mut field: axum::extract::multipart::Field<'_>,
) -> anyhow::Result<PathBuf> {
    let upload_dir = state
        .staging_dir
        .join("uploads")
        .join(project_id.to_string());
    tokio::fs::create_dir_all(&upload_dir).await?;
    let target = upload_dir.join(filename);
    let mut file = tokio::fs::File::create(&target).await?;
    while let Some(chunk) = field.chunk().await? {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    Ok(target)
}

async fn upload_media(
    State(state): State<AppState>,
    Path(id): Path<String>,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    upload_file(state, id, multipart, "media").await
}

async fn upload_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    upload_file(state, id, multipart, "archive").await
}

async fn import_dataset(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<DatasetImportResponse>, (StatusCode, Json<ErrorResponse>)> {
    let project_id = parse_uuid(&id)?;
    let mut file_path: Option<PathBuf> = None;
    let mut audio_path_field = String::from("audio_path");
    let mut selected_manifest_path: Option<PathBuf> = None;
    while let Some(field) = multipart.next_field().await.map_err(|err| {
        error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
    })? {
        match field.name().unwrap_or_default() {
            "file" => {
                let filename = field.file_name().unwrap_or("dataset");
                let filename = filename.to_string();
                let path = persist_staged_upload(&state, project_id, filename, field)
                    .await
                    .map_err(|err| {
                        error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
                    })?;
                file_path = Some(path);
            }
            "audio_path_field" => {
                audio_path_field = field.text().await.unwrap_or(audio_path_field);
            }
            "selected_manifest_path" => {
                let value = field.text().await.unwrap_or_default();
                if !value.trim().is_empty() {
                    selected_manifest_path = Some(PathBuf::from(value));
                }
            }
            _ => {}
        }
    }
    let file_path = file_path.ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "missing file upload",
        )
    })?;
    let summary = match file_path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
    {
        Some("csv") | Some("jsonl") | Some("json") | Some("parquet") => {
            let imported = ingestion::import_dataset_manifest(
                &state.database,
                project_id,
                &file_path,
                &audio_path_field,
            )
            .await
            .map_err(|err| {
                error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
            })?;
            ingestion::CsvImportSummary {
                imported: imported.len() as u32,
                duplicate: 0,
                invalid: 0,
                failed: 0,
                row_failures: vec![],
            }
        }
        Some("zip") | Some("tar") | Some("gz") | Some("tgz") => {
            let results = ingestion::import_archive_dataset(
                &state.database,
                project_id,
                &file_path,
                selected_manifest_path.as_deref(),
                &audio_path_field,
                &state.staging_dir,
            )
            .await
            .map_err(|err| {
                error_response(StatusCode::BAD_REQUEST, "invalid_request", err.to_string())
            })?;
            crate::ingestion::CsvImportSummary {
                imported: results.len() as u32,
                duplicate: 0,
                invalid: 0,
                failed: 0,
                row_failures: vec![],
            }
        }
        _ => {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "unsupported dataset format",
            ))
        }
    };
    Ok(Json(DatasetImportResponse {
        imported: summary.imported,
        duplicate: summary.duplicate,
        invalid: summary.invalid,
        failed: summary.failed,
        row_failures: summary
            .row_failures
            .into_iter()
            .map(|failure| RowFailureResponse {
                row: failure.row,
                reason: failure.reason,
            })
            .collect(),
    }))
}

impl From<crate::project::Project> for ProjectResponse {
    fn from(value: crate::project::Project) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            description: value.description,
            status: value.status.as_str().to_string(),
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

fn parse_rfc3339(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&chrono::Utc))
}

fn parse_source_type(value: &str) -> Result<SourceType, (StatusCode, Json<ErrorResponse>)> {
    match value {
        "local_file" => Ok(SourceType::LocalFile),
        "local_folder" => Ok(SourceType::LocalFolder),
        "public_url" => Ok(SourceType::PublicUrl),
        "csv_import" => Ok(SourceType::CsvImport),
        "existing_dataset" => Ok(SourceType::ExistingDataset),
        "hugging_face_dataset" => Ok(SourceType::HuggingFaceDataset),
        "archive" => Ok(SourceType::Archive),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "invalid source type",
        )),
    }
}

fn parse_job_type(value: &str) -> Result<JobType, (StatusCode, Json<ErrorResponse>)> {
    match value {
        "source_registration" => Ok(JobType::SourceRegistration),
        "media_ingestion" => Ok(JobType::MediaIngestion),
        "audio_extraction" => Ok(JobType::AudioExtraction),
        "audio_validation" => Ok(JobType::AudioValidation),
        "video_cleanup" => Ok(JobType::VideoCleanup),
        "duplicate_detection" => Ok(JobType::DuplicateDetection),
        "ai_analysis" => Ok(JobType::AiAnalysis),
        "interval_mining" => Ok(JobType::IntervalMining),
        "manual_review" => Ok(JobType::ManualReview),
        "dataset_export" => Ok(JobType::DatasetExport),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "invalid job type",
        )),
    }
}

fn parse_job_status(value: &str) -> Result<JobStatus, (StatusCode, Json<ErrorResponse>)> {
    match value {
        "queued" => Ok(JobStatus::Queued),
        "running" => Ok(JobStatus::Running),
        "completed" => Ok(JobStatus::Completed),
        "failed" => Ok(JobStatus::Failed),
        "cancelled" => Ok(JobStatus::Cancelled),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "invalid job status",
        )),
    }
}

fn source_response(source: crate::source::Source) -> SourceResponse {
    let progress = source.ingestion_status.as_str().to_string();
    SourceResponse {
        id: source.id.to_string(),
        project_id: source.project_id.to_string(),
        source_type: source_type_name(source.source_type).to_string(),
        original_url: source.original_url,
        source_config: source.source_config,
        failure_reason: source.failure_reason.clone(),
        import_metadata: source.import_metadata.clone(),
        file_count: source.file_count,
        byte_count: source.byte_count,
        ingestion_status: progress.clone(),
        progress,
        failure_summary: bounded_summary(source.failure_reason.as_deref()),
        created_at: source.created_at.to_rfc3339(),
        updated_at: source.updated_at.to_rfc3339(),
    }
}

fn job_response(job: crate::job::Job) -> JobResponse {
    let status = job_status_name(job.status).to_string();
    JobResponse {
        id: job.id.to_string(),
        project_id: job.project_id.to_string(),
        source_id: job.source_id.map(|id| id.to_string()),
        job_type: job_type_name(job.job_type).to_string(),
        status: status.clone(),
        priority: job.priority,
        attempts: job.attempts,
        progress: status,
        failure_summary: bounded_summary(job.failure_reason.as_deref()),
        created_at: job.created_at.to_rfc3339(),
        updated_at: job.updated_at.to_rfc3339(),
    }
}

fn media_asset_response(media_asset: MediaAsset) -> MediaAssetResponse {
    MediaAssetResponse {
        id: media_asset.id.to_string(),
        project_id: media_asset.project_id.to_string(),
        source_id: media_asset.source_id.to_string(),
        media_kind: media_asset.media_kind,
        staged_path: media_asset.staged_path,
        original_filename: media_asset.original_filename,
        byte_size: media_asset.byte_size,
        ingestion_status: media_asset.ingestion_status,
        inspection_status: media_asset.inspection_status,
        extraction_status: media_asset.extraction_status,
        extraction_progress: media_asset.extraction_progress,
        extraction_tmp_path: media_asset.extraction_tmp_path,
        extracted_at: media_asset.extracted_at.map(|value| value.to_rfc3339()),
        container_format: media_asset.container_format,
        duration_seconds: media_asset.duration_seconds,
        codecs: media_asset.codecs,
        sample_rate_hz: media_asset.sample_rate_hz,
        channels: media_asset.channels,
        inspected_at: media_asset.inspected_at.map(|value| value.to_rfc3339()),
        created_at: media_asset.created_at.to_rfc3339(),
        updated_at: media_asset.updated_at.to_rfc3339(),
    }
}

fn audio_asset_response(
    audio_asset: crate::audio_asset::AudioAsset,
    source_id: Option<Uuid>,
) -> AudioAssetResponse {
    AudioAssetResponse {
        id: audio_asset.id.to_string(),
        project_id: audio_asset.project_id.to_string(),
        media_asset_id: audio_asset.media_asset_id.to_string(),
        source_id: source_id.map(|value| value.to_string()),
        processing_status: audio_asset.processing_status,
        normalization_profile: audio_asset.normalization_profile,
        validation_status: audio_asset.validation_status,
        validation_error: audio_asset.validation_error,
        validated_at: audio_asset.validated_at.map(|value| value.to_rfc3339()),
        duration_seconds: audio_asset.duration_seconds,
        audio_format: audio_asset.audio_format,
        codec: audio_asset.codec,
        sample_rate_hz: audio_asset.sample_rate_hz,
        channels: audio_asset.channels,
        byte_size: audio_asset.byte_size,
        storage_path: audio_asset.storage_path,
        created_at: audio_asset.created_at.to_rfc3339(),
        updated_at: audio_asset.updated_at.to_rfc3339(),
    }
}

async fn resolve_audio_asset_source_id(
    state: &AppState,
    media_asset_id: Uuid,
) -> Result<Option<Uuid>, (StatusCode, Json<ErrorResponse>)> {
    let media_asset = state
        .database
        .media_assets()
        .get(media_asset_id)
        .await
        .map_err(database_error)?;
    Ok(media_asset.map(|asset| asset.source_id))
}

fn source_type_name(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::LocalFile => "local_file",
        SourceType::LocalFolder => "local_folder",
        SourceType::PublicUrl => "public_url",
        SourceType::CsvImport => "csv_import",
        SourceType::ExistingDataset => "existing_dataset",
        SourceType::HuggingFaceDataset => "hugging_face_dataset",
        SourceType::Archive => "archive",
    }
}

fn job_type_name(job_type: JobType) -> &'static str {
    match job_type {
        JobType::SourceRegistration => "source_registration",
        JobType::MediaIngestion => "media_ingestion",
        JobType::AudioExtraction => "audio_extraction",
        JobType::AudioValidation => "audio_validation",
        JobType::VideoCleanup => "video_cleanup",
        JobType::DuplicateDetection => "duplicate_detection",
        JobType::AiAnalysis => "ai_analysis",
        JobType::IntervalMining => "interval_mining",
        JobType::ManualReview => "manual_review",
        JobType::DatasetExport => "dataset_export",
        JobType::MediaInspection => "media_inspection",
    }
}

fn job_status_name(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn bounded_summary(value: Option<&str>) -> Option<String> {
    value.map(|value| value.chars().take(256).collect())
}
