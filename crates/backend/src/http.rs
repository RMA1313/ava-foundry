use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    audio_asset::AudioAssetListFilter,
    duplicate::DuplicateGroupDetails,
    ingestion,
    job::{JobListFilter, JobStatus, JobType},
    server::AppState,
    source::{SourceListFilter, SourceType},
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
struct AudioAssetsResponse {
    items: Vec<AudioAssetResponse>,
    limit: u32,
    offset: u32,
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
        .route("/projects/:id", get(get_project).put(update_project))
        .route("/projects/:id/sources", get(list_sources))
        .route("/projects/:id/jobs", get(list_jobs))
        .route("/projects/:id/audio-assets", get(list_audio_assets))
        .route(
            "/projects/:id/audio-assets/:audio_asset_id",
            get(get_audio_asset),
        )
        .route("/projects/:id/duplicates", get(list_duplicate_groups))
        .route("/projects/:id/metrics/summary", get(metrics_summary))
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
    Ok(Json(MetricsSummaryResponse {
        stage_runs: runs.into_iter().map(stage_run_response).collect(),
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
