use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    ingestion,
    job::{JobListFilter, JobStatus, JobType},
    server::AppState,
    source::{SourceListFilter, SourceType},
};

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

#[derive(Deserialize)]
struct SourceListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    source_type: Option<String>,
    ingestion_status: Option<String>,
}

#[derive(Deserialize)]
struct JobListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    job_type: Option<String>,
    status: Option<String>,
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

#[derive(Serialize)]
struct SourceResponse {
    id: String,
    project_id: String,
    source_type: String,
    original_url: Option<String>,
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
        .route("/projects/:id", get(get_project).put(update_project))
        .route("/projects/:id/sources", get(list_sources))
        .route("/projects/:id/jobs", get(list_jobs))
        .route("/projects/:project_id/jobs/:job_id/retry", post(retry_job))
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
    let page = state
        .database
        .sources()
        .list_paginated(
            project_id,
            Some(SourceListFilter {
                source_type,
                ingestion_status: query.ingestion_status.clone(),
            }),
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
    let progress = source.ingestion_status.clone();
    SourceResponse {
        id: source.id.to_string(),
        project_id: source.project_id.to_string(),
        source_type: source_type_name(source.source_type).to_string(),
        original_url: source.original_url,
        ingestion_status: progress.clone(),
        progress,
        failure_summary: bounded_summary(source.import_metadata.as_deref()),
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
