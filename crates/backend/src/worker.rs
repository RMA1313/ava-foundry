#![allow(clippy::items_after_test_module)]

use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::info;

use crate::{
    config::AudioNormalizationProfile,
    download::DownloadCommandAdapter,
    ffmpeg::FfmpegAdapter,
    ffprobe::{FfprobeAdapter, FfprobeOutput},
    ingestion,
    job::{Job, JobType},
    server::AppState,
};

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<JobOutcome>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    Completed,
    Retry {
        failure_reason: String,
        retry_after: chrono::DateTime<Utc>,
    },
    Failed {
        failure_reason: String,
    },
}

pub trait JobHandler: Send + Sync + 'static {
    fn handle(&self, job: Job, state: AppState) -> HandlerFuture;
}

pub trait FilesystemMetrics: Send + Sync + 'static {
    fn free_bytes(&self, path: &Path) -> Result<u64>;
    fn total_bytes(&self, path: &Path) -> Result<u64>;
}

#[derive(Debug, Default)]
pub struct DefaultFilesystemMetrics;

impl FilesystemMetrics for DefaultFilesystemMetrics {
    fn free_bytes(&self, path: &Path) -> Result<u64> {
        filesystem_bytes(path).map(|(free, _)| free)
    }

    fn total_bytes(&self, path: &Path) -> Result<u64> {
        filesystem_bytes(path).map(|(_, total)| total)
    }
}

#[derive(Clone)]
pub struct StagingCapacityGuard {
    pause_free_bytes: u64,
    resume_free_bytes: u64,
    metrics: Arc<dyn FilesystemMetrics>,
    paused: Arc<AtomicBool>,
}

impl StagingCapacityGuard {
    pub fn new(
        pause_free_bytes: u64,
        resume_free_bytes: u64,
        metrics: impl FilesystemMetrics,
    ) -> Self {
        Self {
            pause_free_bytes,
            resume_free_bytes,
            metrics: Arc::new(metrics),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn check(&self, path: &Path) -> Result<StagingCapacityState> {
        let free_bytes = self.metrics.free_bytes(path)?;
        let total_bytes = self.metrics.total_bytes(path)?;
        let currently_paused = self.paused.load(Ordering::SeqCst);
        let next_paused = if currently_paused {
            free_bytes < self.resume_free_bytes
        } else {
            free_bytes < self.pause_free_bytes
        };
        self.paused.store(next_paused, Ordering::SeqCst);
        Ok(StagingCapacityState {
            paused: next_paused,
            free_bytes,
            total_bytes,
            pause_free_bytes: self.pause_free_bytes,
            resume_free_bytes: self.resume_free_bytes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagingCapacityState {
    pub paused: bool,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub pause_free_bytes: u64,
    pub resume_free_bytes: u64,
}

#[cfg(unix)]
fn filesystem_bytes(path: &Path) -> Result<(u64, u64)> {
    use std::process::Command;
    let output = Command::new("df").arg("-Pk").arg(path).output()?;
    if !output.status.success() {
        return Err(anyhow!("failed to read filesystem capacity"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| anyhow!("failed to parse filesystem capacity"))?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 6 {
        return Err(anyhow!("failed to parse filesystem capacity"));
    }
    let total_kib = fields[1].parse::<u64>()?;
    let avail_kib = fields[3].parse::<u64>()?;
    Ok((avail_kib * 1024, total_kib * 1024))
}

#[cfg(not(unix))]
fn filesystem_bytes(_path: &Path) -> Result<(u64, u64)> {
    Err(anyhow!(
        "filesystem metrics are unsupported on this platform"
    ))
}

impl<F, Fut> JobHandler for F
where
    F: Fn(Job, AppState) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<JobOutcome>> + Send + 'static,
{
    fn handle(&self, job: Job, state: AppState) -> HandlerFuture {
        Box::pin((self)(job, state))
    }
}

#[derive(Clone)]
pub struct WorkerRuntime {
    worker_id: String,
    poll_interval: std::time::Duration,
    lease_duration: Duration,
    max_attempts: i32,
    concurrency_limit: usize,
    staging_dir: Option<PathBuf>,
    staging_capacity_guard: Option<StagingCapacityGuard>,
    handlers: Arc<HashMap<JobType, Arc<dyn JobHandler>>>,
}

impl WorkerRuntime {
    pub fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            poll_interval: std::time::Duration::from_millis(25),
            lease_duration: Duration::minutes(5),
            max_attempts: 3,
            concurrency_limit: 1,
            staging_dir: None,
            staging_capacity_guard: None,
            handlers: Arc::new(HashMap::new()),
        }
    }

    pub fn with_handler(mut self, job_type: JobType, handler: impl JobHandler) -> Self {
        let mut handlers = (*self.handlers).clone();
        handlers.insert(job_type, Arc::new(handler));
        self.handlers = Arc::new(handlers);
        self
    }

    pub fn with_concurrency_limit(mut self, concurrency_limit: usize) -> Self {
        self.concurrency_limit = concurrency_limit.max(1);
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: std::time::Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn with_lease_duration(mut self, lease_duration: Duration) -> Self {
        self.lease_duration = lease_duration;
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    pub fn with_staging_capacity_guard(
        mut self,
        staging_dir: impl Into<PathBuf>,
        guard: StagingCapacityGuard,
    ) -> Self {
        self.staging_dir = Some(staging_dir.into());
        self.staging_capacity_guard = Some(guard);
        self
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub async fn run(
        &self,
        state: AppState,
        shutdown: impl Future<Output = ()> + Send,
    ) -> Result<()> {
        let stopping = Arc::new(AtomicBool::new(false));
        let stop_flag = stopping.clone();
        tokio::pin!(shutdown);
        let mut join_set = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    stop_flag.store(true, Ordering::SeqCst);
                }
                _ = tokio::time::sleep(self.poll_interval), if !stop_flag.load(Ordering::SeqCst) => {
                    self.claim_and_spawn(state.clone(), &mut join_set, semaphore.clone()).await?;
                }
                result = join_set.join_next(), if !join_set.is_empty() => {
                    if let Some(result) = result {
                        result??;
                    }
                }
            }

            if stop_flag.load(Ordering::SeqCst) && join_set.is_empty() {
                break;
            }
        }
        Ok(())
    }

    async fn claim_and_spawn(
        &self,
        state: AppState,
        join_set: &mut JoinSet<Result<()>>,
        semaphore: Arc<Semaphore>,
    ) -> Result<()> {
        while semaphore.available_permits() > 0 {
            if let (Some(staging_dir), Some(guard)) =
                (&self.staging_dir, &self.staging_capacity_guard)
            {
                let capacity = guard.check(staging_dir)?;
                if capacity.paused {
                    info!(
                        free_bytes = capacity.free_bytes,
                        total_bytes = capacity.total_bytes,
                        pause_free_bytes = capacity.pause_free_bytes,
                        resume_free_bytes = capacity.resume_free_bytes,
                        "staging capacity guard paused new claims"
                    );
                    break;
                }
            }
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => break,
            };
            let lease_expires_at = Utc::now() + self.lease_duration;
            let claimed = state
                .database
                .jobs()
                .claim_next(&self.worker_id, lease_expires_at)
                .await?;
            let Some(job) = claimed else {
                drop(permit);
                break;
            };
            let handlers = self.handlers.clone();
            let state_clone = state.clone();
            let runtime = self.clone();
            join_set.spawn(async move {
                let _permit = permit;
                runtime.execute_job(state_clone, handlers, job).await
            });
        }
        Ok(())
    }

    async fn execute_job(
        self,
        state: AppState,
        handlers: Arc<HashMap<JobType, Arc<dyn JobHandler>>>,
        job: Job,
    ) -> Result<()> {
        let handler = match handlers.get(&job.job_type).cloned() {
            Some(handler) => handler,
            None => {
                let _ = state
                    .database
                    .jobs()
                    .fail(job.id, &format!("unknown job type: {:?}", job.job_type))
                    .await?;
                return Ok(());
            }
        };
        let outcome = handler.handle(job.clone(), state.clone()).await?;
        match outcome {
            JobOutcome::Completed => {
                if job.job_type != JobType::MediaIngestion
                    && job.job_type != JobType::AudioValidation
                {
                    let _ = state.database.jobs().complete(job.id).await?;
                }
            }
            JobOutcome::Failed { failure_reason } => {
                let _ = state.database.jobs().fail(job.id, &failure_reason).await?;
            }
            JobOutcome::Retry {
                failure_reason,
                retry_after,
            } => {
                let _ = state.database.jobs().fail(job.id, &failure_reason).await?;
                let _ = state
                    .database
                    .jobs()
                    .schedule_retry(job.id, &failure_reason, retry_after, self.max_attempts)
                    .await?;
            }
        }
        Ok(())
    }
}

pub fn media_ingestion_handler(
    staging_dir: impl Into<PathBuf>,
    download_adapter: DownloadCommandAdapter,
) -> impl JobHandler {
    let staging_dir = staging_dir.into();
    move |job: Job, state: AppState| {
        let staging_dir = staging_dir.clone();
        let download_adapter = download_adapter.clone();
        async move { handle_media_ingestion_job(&state, job, &staging_dir, &download_adapter).await }
    }
}

pub fn media_inspection_handler(ffprobe_path: impl Into<PathBuf>) -> impl JobHandler {
    let ffprobe_path = ffprobe_path.into();
    move |job: Job, state: AppState| {
        let ffprobe_path = ffprobe_path.clone();
        async move { handle_media_inspection_job(&state, job, &ffprobe_path).await }
    }
}

pub fn audio_extraction_handler(ffmpeg_path: impl Into<PathBuf>) -> impl JobHandler {
    let ffmpeg_path = ffmpeg_path.into();
    move |job: Job, state: AppState| {
        let ffmpeg_path = ffmpeg_path.clone();
        async move { handle_audio_extraction_job(&state, job, &ffmpeg_path).await }
    }
}

pub fn audio_validation_handler(
    ffmpeg_path: impl Into<PathBuf>,
    ffprobe_path: impl Into<PathBuf>,
) -> impl JobHandler {
    let ffmpeg_path = ffmpeg_path.into();
    let ffprobe_path = ffprobe_path.into();
    move |job: Job, state: AppState| {
        let ffmpeg_path = ffmpeg_path.clone();
        let ffprobe_path = ffprobe_path.clone();
        async move { handle_audio_validation_job(&state, job, &ffmpeg_path, &ffprobe_path).await }
    }
}

async fn handle_media_ingestion_job(
    state: &AppState,
    job: Job,
    staging_dir: &Path,
    download_adapter: &DownloadCommandAdapter,
) -> Result<JobOutcome> {
    let source_id = job
        .source_id
        .ok_or_else(|| anyhow!("media ingestion job requires a source"))?;
    let source = state
        .database
        .sources()
        .get(source_id)
        .await?
        .ok_or_else(|| anyhow!("source not found"))?;
    match source.source_type {
        crate::source::SourceType::LocalFile => {
            let source_path = source
                .original_url
                .as_deref()
                .ok_or_else(|| anyhow!("local file source missing path"))?;
            let (staged_path, byte_size) = ingestion::import_local_file_to_staging(
                &state.database,
                source_id,
                Path::new(source_path),
                staging_dir,
            )
            .await?;
            finalize_media_ingestion(
                &state.database,
                job.id,
                source_id,
                "file",
                staged_path.as_path(),
                byte_size as i64,
                source.original_url.as_deref(),
            )
            .await?;
            Ok(JobOutcome::Completed)
        }
        crate::source::SourceType::PublicUrl => {
            let url = source
                .original_url
                .as_deref()
                .ok_or_else(|| anyhow!("public url source missing url"))?;
            let _staged = download_url_to_staging(
                &state.database,
                source_id,
                url,
                staging_dir,
                download_adapter,
            )
            .await?;
            let staged_path = staging_dir.join(format!("{}.bin", source_id));
            let byte_size = staged_path.metadata()?.len();
            finalize_media_ingestion(
                &state.database,
                job.id,
                source_id,
                "video",
                staged_path.as_path(),
                byte_size as i64,
                source.original_url.as_deref(),
            )
            .await?;
            Ok(JobOutcome::Completed)
        }
        other => Ok(JobOutcome::Failed {
            failure_reason: format!("unsupported source type: {:?}", other),
        }),
    }
}

async fn handle_media_inspection_job(
    state: &AppState,
    job: Job,
    ffprobe_path: &Path,
) -> Result<JobOutcome> {
    let media_asset_id = job
        .source_id
        .ok_or_else(|| anyhow!("media inspection job requires a media asset id"))?;
    let media_asset = state
        .database
        .media_assets()
        .get_by_source(media_asset_id)
        .await?
        .ok_or_else(|| anyhow!("media asset not found"))?;
    let adapter = match FfprobeAdapter::new(ffprobe_path) {
        Ok(adapter) => adapter,
        Err(err) => {
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
    };
    let probed = match adapter
        .probe(
            Path::new(&media_asset.staged_path),
            std::time::Duration::from_secs(5),
        )
        .await
    {
        Ok(output) => output,
        Err(err) if err.to_string().contains("timed out") => {
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
        Err(err) if err.to_string().contains("parse ffprobe json") => {
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
        Err(err) if err.to_string().contains("ffprobe failed") => {
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
        Err(err) => {
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
    };
    persist_media_inspection(&state.database, &media_asset, &probed).await
}

async fn persist_media_inspection(
    database: &crate::db::Database,
    media_asset: &crate::media_asset::MediaAsset,
    probed: &FfprobeOutput,
) -> Result<JobOutcome> {
    let container_format = probed
        .format
        .as_ref()
        .and_then(|format| format.format_name.clone());
    let duration_seconds = probed
        .format
        .as_ref()
        .and_then(|format| format.duration.as_deref())
        .and_then(|value| value.parse::<f64>().ok());
    let audio_streams: Vec<_> = probed
        .streams
        .iter()
        .filter(|stream| stream.codec_type.as_deref() == Some("audio"))
        .collect();
    let inspection_status = if audio_streams.is_empty() {
        "no_audio_stream"
    } else {
        "inspectable"
    };
    if audio_streams.is_empty() {
        let _ = database
            .media_assets()
            .upsert_ingested(
                database.pool(),
                crate::media_asset::MediaAssetUpsert {
                    project_id: media_asset.project_id,
                    source_id: media_asset.source_id,
                    media_kind: media_asset.media_kind.clone(),
                    staged_path: media_asset.staged_path.clone(),
                    original_filename: media_asset.original_filename.clone(),
                    byte_size: media_asset.byte_size,
                    ingestion_status: media_asset.ingestion_status.clone(),
                    inspection_status: Some(inspection_status.to_string()),
                    extraction_status: media_asset.extraction_status.clone(),
                    extraction_progress: media_asset.extraction_progress.clone(),
                    extraction_tmp_path: media_asset.extraction_tmp_path.clone(),
                    extracted_at: media_asset.extracted_at,
                    container_format,
                    duration_seconds,
                    codecs: Some(
                        probed
                            .streams
                            .iter()
                            .filter_map(|stream| stream.codec_name.clone())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    sample_rate_hz: None,
                    channels: None,
                    inspected_at: Some(Utc::now()),
                },
            )
            .await?;
        return Ok(JobOutcome::Failed {
            failure_reason: "media has no audio stream".to_string(),
        });
    }
    let audio = audio_streams[0];
    let sample_rate_hz = audio
        .sample_rate
        .as_deref()
        .and_then(|value| value.parse::<i32>().ok());
    let channels = audio.channels;
    let _ = database
        .media_assets()
        .upsert_ingested(
            database.pool(),
            crate::media_asset::MediaAssetUpsert {
                project_id: media_asset.project_id,
                source_id: media_asset.source_id,
                media_kind: media_asset.media_kind.clone(),
                staged_path: media_asset.staged_path.clone(),
                original_filename: media_asset.original_filename.clone(),
                byte_size: media_asset.byte_size,
                ingestion_status: media_asset.ingestion_status.clone(),
                inspection_status: Some(inspection_status.to_string()),
                extraction_status: media_asset.extraction_status.clone(),
                extraction_progress: media_asset.extraction_progress.clone(),
                extraction_tmp_path: media_asset.extraction_tmp_path.clone(),
                extracted_at: media_asset.extracted_at,
                container_format,
                duration_seconds,
                codecs: Some(
                    probed
                        .streams
                        .iter()
                        .filter_map(|stream| stream.codec_name.clone())
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                sample_rate_hz,
                channels,
                inspected_at: Some(Utc::now()),
            },
        )
        .await?;
    Ok(JobOutcome::Completed)
}

pub async fn finalize_media_ingestion(
    database: &crate::db::Database,
    job_id: uuid::Uuid,
    source_id: uuid::Uuid,
    media_kind: &str,
    staged_path: &Path,
    byte_size: i64,
    original_filename: Option<&str>,
) -> Result<()> {
    let mut tx = database.pool().begin().await?;
    let source = database
        .sources()
        .get(source_id)
        .await?
        .ok_or_else(|| anyhow!("source not found"))?;
    let source_update = sqlx::query(
        r#"
        update sources
        set ingestion_status = 'completed',
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(source_id)
    .execute(&mut *tx)
    .await?;
    if source_update.rows_affected() != 1 {
        return Err(anyhow!("source not found"));
    }
    database
        .media_assets()
        .upsert_ingested(
            &mut *tx,
            crate::media_asset::MediaAssetUpsert {
                project_id: source.project_id,
                source_id,
                media_kind: media_kind.to_string(),
                staged_path: staged_path.to_string_lossy().to_string(),
                original_filename: original_filename.map(str::to_string),
                byte_size,
                ingestion_status: "ready".to_string(),
                inspection_status: None,
                extraction_status: None,
                extraction_progress: None,
                extraction_tmp_path: None,
                extracted_at: None,
                container_format: None,
                duration_seconds: None,
                codecs: None,
                sample_rate_hz: None,
                channels: None,
                inspected_at: None,
            },
        )
        .await?;
    let job_update = sqlx::query(
        r#"
        update jobs
        set status = 'completed',
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .execute(&mut *tx)
    .await?;
    if job_update.rows_affected() != 1 {
        return Err(anyhow!("job not found"));
    }
    tx.commit().await?;
    Ok(())
}

async fn download_url_to_staging(
    _database: &crate::db::Database,
    source_id: uuid::Uuid,
    url: &str,
    staging_dir: &Path,
    adapter: &DownloadCommandAdapter,
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(staging_dir).await?;
    let final_path = staging_dir.join(format!("{}.bin", source_id));
    if final_path.exists() {
        return Ok(final_path);
    }
    let tmp_path = staging_dir.join(format!("{}.part", source_id));
    let mut process = adapter.start(url, &tmp_path).await?;
    while let Some(_line) = process.next_progress().await {}
    let status = process.wait().await?;
    if !status.success() {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(anyhow!("download process failed"));
    }
    tokio::fs::rename(&tmp_path, &final_path).await?;
    Ok(final_path)
}

pub fn generate_worker_id() -> String {
    format!("worker-{}", Uuid::new_v4().simple())
}

use uuid::Uuid;

pub fn default_worker_runtime() -> WorkerRuntime {
    WorkerRuntime::new(generate_worker_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Database,
        job::{JobRepository, JobStatus},
        migrations,
        storage::AudioStorageConfig,
    };
    use chrono::Utc;
    use sqlx::PgPool;
    use std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::atomic::AtomicU64,
    };
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    fn clear_env() {
        for key in [
            "TMPDIR",
            "TEMP",
            "TMP",
            "DATA_STUDIO_DATABASE_URL",
            "DATA_STUDIO_WORKSPACE_DIR",
            "DATA_STUDIO_DATA_DIR",
            "DATA_STUDIO_TEMP_DIR",
            "DATA_STUDIO_AUDIO_DIR",
            "DATA_STUDIO_AUDIO_TEMP_DIR",
            "DATA_STUDIO_STAGING_PAUSE_FREE_BYTES",
            "DATA_STUDIO_STAGING_RESUME_FREE_BYTES",
            "DATA_STUDIO_HOST",
            "DATA_STUDIO_PORT",
        ] {
            std::env::remove_var(key);
        }
    }

    #[derive(Clone, Default)]
    struct FakeFilesystemMetrics {
        free_bytes: Arc<AtomicU64>,
        total_bytes: Arc<AtomicU64>,
    }

    impl FakeFilesystemMetrics {
        fn new(free_bytes: u64, total_bytes: u64) -> Self {
            Self {
                free_bytes: Arc::new(AtomicU64::new(free_bytes)),
                total_bytes: Arc::new(AtomicU64::new(total_bytes)),
            }
        }

        fn set_free_bytes(&self, free_bytes: u64) {
            self.free_bytes.store(free_bytes, Ordering::SeqCst);
        }
    }

    impl FilesystemMetrics for FakeFilesystemMetrics {
        fn free_bytes(&self, _path: &Path) -> Result<u64> {
            Ok(self.free_bytes.load(Ordering::SeqCst))
        }

        fn total_bytes(&self, _path: &Path) -> Result<u64> {
            Ok(self.total_bytes.load(Ordering::SeqCst))
        }
    }

    async fn test_pool() -> PgPool {
        let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
            .unwrap_or_else(|_| {
                "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
            });
        let schema_name = format!("worker_test_{}", Uuid::new_v4().simple());
        let admin_pool = PgPool::connect(&database_url)
            .await
            .expect("failed to connect to test database");
        sqlx::query(&format!("create schema {}", schema_name))
            .execute(&admin_pool)
            .await
            .expect("failed to create test schema");
        let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
        migrations::run(&schema_database_url)
            .await
            .expect("failed to run migrations");
        PgPool::connect(&schema_database_url)
            .await
            .expect("failed to connect to schema-scoped test database")
    }

    async fn seed_project(pool: &PgPool) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into projects (id, name, description, created_at, updated_at)
            values ($1, $2, $3, now(), now())
            "#,
        )
        .bind(id)
        .bind("Research")
        .bind(Option::<String>::None)
        .execute(pool)
        .await
        .expect("project should insert");
        id
    }

    #[tokio::test]
    async fn unknown_job_types_fail_deterministically() {
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool);
        let source_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, now(), now())
            "#,
        )
        .bind(source_id)
        .bind(project_id)
        .bind("public_url")
        .bind(Some("https://example.com/video"))
        .bind("url:https://example.com/video")
        .bind(Option::<String>::None)
        .bind("pending")
        .execute(database.pool())
        .await
        .expect("source should insert");
        let _created = JobRepository::new(database.pool().clone())
            .create(
                project_id,
                Some(source_id),
                JobType::AudioExtraction,
                0,
                Utc::now(),
            )
            .await
            .expect("job should create");
        let job = JobRepository::new(database.pool().clone())
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .expect("claim should succeed")
            .expect("job should be claimable");
        let runtime = WorkerRuntime::new("worker-test".to_string());
        let outcome = runtime
            .execute_job(
                AppState {
                    database: database.clone(),
                    staging_dir: PathBuf::from("/tmp/staging"),
                    audio_storage: AudioStorageConfig::new(
                        PathBuf::from("/tmp/audio"),
                        PathBuf::from("/tmp/audio-tmp"),
                    ),
                    audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(
                    ),
                    staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                        1,
                        2,
                        DefaultFilesystemMetrics,
                    )),
                },
                Arc::new(HashMap::new()),
                job.clone(),
            )
            .await;
        assert!(outcome.is_ok());
        let fetched = database.jobs().get(job.id).await.unwrap().unwrap();
        assert_eq!(fetched.status, JobStatus::Failed);
        assert!(fetched
            .failure_reason
            .as_deref()
            .unwrap()
            .contains("unknown job type"));
    }

    #[tokio::test]
    async fn dispatches_fake_handlers() {
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool);
        let source_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, now(), now())
            "#,
        )
        .bind(source_id)
        .bind(project_id)
        .bind("public_url")
        .bind(Some("https://example.com/video"))
        .bind("url:https://example.com/video")
        .bind(Option::<String>::None)
        .bind("pending")
        .execute(database.pool())
        .await
        .expect("source should insert");
        let _created = JobRepository::new(database.pool().clone())
            .create(
                project_id,
                Some(source_id),
                JobType::AudioExtraction,
                0,
                Utc::now(),
            )
            .await
            .expect("job should create");
        let job = JobRepository::new(database.pool().clone())
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .expect("claim should succeed")
            .expect("job should be claimable");
        let calls = Arc::new(Mutex::new(0usize));
        let handler_calls = calls.clone();
        let runtime = WorkerRuntime::new("worker-test".to_string()).with_handler(
            JobType::AudioExtraction,
            move |_job, _state| {
                let handler_calls = handler_calls.clone();
                async move {
                    *handler_calls.lock().await += 1;
                    Ok(JobOutcome::Completed)
                }
            },
        );
        let handlers = runtime.handlers.clone();
        runtime
            .execute_job(
                AppState {
                    database,
                    staging_dir: PathBuf::from("/tmp/staging"),
                    audio_storage: AudioStorageConfig::new(
                        PathBuf::from("/tmp/audio"),
                        PathBuf::from("/tmp/audio-tmp"),
                    ),
                    audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(
                    ),
                    staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                        1,
                        2,
                        DefaultFilesystemMetrics,
                    )),
                },
                handlers,
                job.clone(),
            )
            .await
            .expect("dispatch should succeed");
        assert_eq!(*calls.lock().await, 1);
    }

    fn fake_ffprobe(dir: &TempDir, json: &str) -> PathBuf {
        let path = dir.path().join("fake-ffprobe");
        fs::write(&path, format!("#!/bin/sh\ncat <<'JSON'\n{}\nJSON\n", json))
            .expect("script should write");
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("permissions");
        path
    }

    async fn seed_media_asset(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
        let project_id = seed_project(pool).await;
        let source_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
            values ($1, $2, $3, $4, $5, $6, $7, now(), now())
            "#,
        )
        .bind(source_id)
        .bind(project_id)
        .bind("public_url")
        .bind(Some("https://example.com/video"))
        .bind("url:https://example.com/video")
        .bind(Option::<String>::None)
        .bind("completed")
        .execute(pool)
        .await
        .expect("source should insert");
        let media_asset_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into media_assets (
                id, project_id, source_id, media_kind, staged_path,
                original_filename, byte_size, ingestion_status,
                inspection_status, container_format, duration_seconds, codecs,
                sample_rate_hz, channels, inspected_at, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, null, null, null, null, null, null, null, now(), now())
            "#,
        )
        .bind(media_asset_id)
        .bind(project_id)
        .bind(source_id)
        .bind("video")
        .bind("/tmp/staged/source.bin")
        .bind(Some("source.mp4"))
        .bind(12_i64)
        .bind("ready")
        .execute(pool)
        .await
        .expect("media asset should insert");
        (project_id, source_id, media_asset_id)
    }

    async fn seed_inspectable_media_asset(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
        let (project_id, source_id, media_asset_id) = seed_media_asset(pool).await;
        sqlx::query(
            r#"
            update media_assets
            set inspection_status = 'inspectable',
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(media_asset_id)
        .execute(pool)
        .await
        .expect("media asset should update");
        (project_id, source_id, media_asset_id)
    }

    fn fake_ffmpeg(dir: &TempDir, script: &str) -> PathBuf {
        let path = dir.path().join("fake-ffmpeg");
        fs::write(&path, script).expect("script should write");
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("permissions");
        path
    }

    fn fake_ffprobe_validation(dir: &TempDir, script: &str) -> PathBuf {
        let path = dir.path().join("fake-ffprobe");
        fs::write(&path, script).expect("script should write");
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("permissions");
        path
    }

    #[tokio::test]
    async fn worker_runtime_persists_inspected_media_metadata() {
        clear_env();
        let pool = test_pool().await;
        let (project_id, source_id, _) = seed_media_asset(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = TempDir::new().expect("temp dir");
        fs::write(temp_dir.path().join("sample.bin"), b"sample").expect("sample");
        let executable = fake_ffprobe(
            &temp_dir,
            r#"{"format":{"format_name":"matroska,webm","duration":"12.5"},"streams":[{"codec_type":"audio","codec_name":"opus","sample_rate":"48000","channels":2}]}"#,
        );
        let _job = database
            .jobs()
            .create(
                project_id,
                Some(source_id),
                JobType::MediaInspection,
                0,
                Utc::now(),
            )
            .await
            .expect("job should be created");
        let job = database
            .jobs()
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .unwrap()
            .unwrap();
        let runtime = WorkerRuntime::new("worker-test".to_string()).with_handler(
            JobType::MediaInspection,
            media_inspection_handler(executable),
        );
        let handlers = runtime.handlers.clone();
        runtime
            .execute_job(
                AppState {
                    database: database.clone(),
                    staging_dir: temp_dir.path().to_path_buf(),
                    audio_storage: AudioStorageConfig::new(
                        PathBuf::from("/tmp/audio"),
                        PathBuf::from("/tmp/audio-tmp"),
                    ),
                    audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(
                    ),
                    staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                        1,
                        2,
                        DefaultFilesystemMetrics,
                    )),
                },
                handlers,
                job,
            )
            .await
            .expect("inspection should succeed");
        let asset = database
            .media_assets()
            .get_by_source(source_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.inspection_status.as_deref(), Some("inspectable"));
        assert_eq!(asset.container_format.as_deref(), Some("matroska,webm"));
        assert_eq!(asset.sample_rate_hz, Some(48000));
        assert_eq!(asset.channels, Some(2));
    }

    #[tokio::test]
    async fn worker_runtime_marks_no_audio_as_terminal_failure_and_is_idempotent() {
        clear_env();
        let pool = test_pool().await;
        let (project_id, source_id, _) = seed_media_asset(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = TempDir::new().expect("temp dir");
        let executable = fake_ffprobe(
            &temp_dir,
            r#"{"format":{"format_name":"matroska,webm","duration":"12.5"},"streams":[{"codec_type":"video","codec_name":"h264"}]}"#,
        );
        let _job = database
            .jobs()
            .create(
                project_id,
                Some(source_id),
                JobType::MediaInspection,
                0,
                Utc::now(),
            )
            .await
            .expect("job should be created");
        let job = database
            .jobs()
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .unwrap()
            .unwrap();
        let runtime = WorkerRuntime::new("worker-test".to_string()).with_handler(
            JobType::MediaInspection,
            media_inspection_handler(executable.clone()),
        );
        let handlers = runtime.handlers.clone();
        let state = AppState {
            database: database.clone(),
            staging_dir: temp_dir.path().to_path_buf(),
            audio_storage: AudioStorageConfig::new(
                PathBuf::from("/tmp/audio"),
                PathBuf::from("/tmp/audio-tmp"),
            ),
            audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
            staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                1,
                2,
                DefaultFilesystemMetrics,
            )),
        };
        runtime
            .clone()
            .execute_job(state.clone(), handlers, job.clone())
            .await
            .expect("inspection should run");
        let first = database
            .media_assets()
            .get_by_source(source_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.inspection_status.as_deref(), Some("no_audio_stream"));
        let runtime = WorkerRuntime::new("worker-test-2".to_string()).with_handler(
            JobType::MediaInspection,
            media_inspection_handler(executable),
        );
        let handlers = runtime.handlers.clone();
        runtime
            .execute_job(state, handlers, job)
            .await
            .expect("reinspection should run");
        let second = database
            .media_assets()
            .get_by_source(source_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.id, first.id);
        assert_eq!(second.inspection_status.as_deref(), Some("no_audio_stream"));
    }

    #[tokio::test]
    async fn worker_runtime_extraction_persists_temporary_output_and_progress() {
        clear_env();
        let pool = test_pool().await;
        let (project_id, source_id, media_asset_id) = seed_inspectable_media_asset(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = TempDir::new().expect("temp dir");
        let executable = fake_ffmpeg(
            &temp_dir,
            "#!/bin/sh\nfor output; do :; done\nprintf 'out_time_ms=1000\\nprogress=continue\\n'\n: > \"$output\"\nprintf 'out_time_ms=2000\\nprogress=end\\n'\n",
        );
        let _job = database
            .jobs()
            .create(
                project_id,
                Some(media_asset_id),
                JobType::AudioExtraction,
                0,
                Utc::now(),
            )
            .await
            .expect("job should be created");
        let job = database
            .jobs()
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .unwrap()
            .unwrap();
        let runtime = WorkerRuntime::new("worker-test".to_string()).with_handler(
            JobType::AudioExtraction,
            audio_extraction_handler(executable),
        );
        let handlers = runtime.handlers.clone();
        let staging_dir = temp_dir.path().join("staging");
        runtime
            .execute_job(
                AppState {
                    database: database.clone(),
                    staging_dir: staging_dir.clone(),
                    audio_storage: AudioStorageConfig::new(
                        PathBuf::from("/tmp/audio"),
                        PathBuf::from("/tmp/audio-tmp"),
                    ),
                    audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(
                    ),
                    staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                        1,
                        2,
                        DefaultFilesystemMetrics,
                    )),
                },
                handlers,
                job,
            )
            .await
            .expect("extraction should succeed");
        let asset = database
            .media_assets()
            .get_by_source(source_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.extraction_status.as_deref(), Some("completed"));
        assert_eq!(asset.extraction_progress.as_deref(), Some("progress=end"));
        assert_eq!(
            asset.extraction_tmp_path.as_deref(),
            Some(
                staging_dir
                    .join(format!("{}.flac.tmp", media_asset_id))
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert!(staging_dir
            .join(format!("{}.flac.tmp", media_asset_id))
            .exists());
    }

    #[tokio::test]
    async fn worker_runtime_removes_abandoned_partial_output_on_retry() {
        clear_env();
        let pool = test_pool().await;
        let (project_id, source_id, media_asset_id) = seed_inspectable_media_asset(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = TempDir::new().expect("temp dir");
        let executable = fake_ffmpeg(
            &temp_dir,
            "#!/bin/sh\nfor output; do :; done\nprintf 'out_time_ms=1000\\nprogress=continue\\n'\nexit 1\n",
        );
        let _job = database
            .jobs()
            .create(
                project_id,
                Some(media_asset_id),
                JobType::AudioExtraction,
                0,
                Utc::now(),
            )
            .await
            .expect("job should be created");
        let job = database
            .jobs()
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .unwrap()
            .unwrap();
        let staging_dir = temp_dir.path().join("staging");
        let runtime = WorkerRuntime::new("worker-test".to_string()).with_handler(
            JobType::AudioExtraction,
            audio_extraction_handler(executable),
        );
        let handlers = runtime.handlers.clone();
        runtime
            .execute_job(
                AppState {
                    database: database.clone(),
                    staging_dir: staging_dir.clone(),
                    audio_storage: AudioStorageConfig::new(
                        PathBuf::from("/tmp/audio"),
                        PathBuf::from("/tmp/audio-tmp"),
                    ),
                    audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(
                    ),
                    staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                        1,
                        2,
                        DefaultFilesystemMetrics,
                    )),
                },
                handlers,
                job,
            )
            .await
            .expect("retry should be recorded");
        assert!(!staging_dir
            .join(format!("{}.flac.tmp", media_asset_id))
            .exists());
        let asset = database
            .media_assets()
            .get_by_source(source_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.extraction_status.as_deref(), Some("failed"));
        assert_eq!(asset.extraction_tmp_path.as_deref(), None);
    }

    async fn seed_validating_media_asset(
        pool: &PgPool,
        temp_dir: &TempDir,
    ) -> (Uuid, Uuid, Uuid, PathBuf) {
        let (project_id, source_id, media_asset_id) = seed_inspectable_media_asset(pool).await;
        let extraction_tmp = temp_dir.path().join(format!("{}.flac.tmp", media_asset_id));
        fs::write(&extraction_tmp, b"raw-audio").expect("extraction temp should write");
        sqlx::query(
            r#"
            update media_assets
            set extraction_tmp_path = $2,
                extraction_status = 'completed',
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(media_asset_id)
        .bind(extraction_tmp.to_string_lossy().to_string())
        .execute(pool)
        .await
        .expect("media asset should update");
        (project_id, source_id, media_asset_id, extraction_tmp)
    }

    #[tokio::test]
    async fn worker_runtime_normalizes_and_validates_audio_with_profile() {
        clear_env();
        let pool = test_pool().await;
        let temp_dir = TempDir::new().expect("temp dir");
        let (project_id, _source_id, media_asset_id, extraction_tmp) =
            seed_validating_media_asset(&pool, &temp_dir).await;
        let database = Database::from_pool(pool.clone());
        let ffmpeg = fake_ffmpeg(
            &temp_dir,
            "#!/bin/sh\nfor arg in \"$@\"; do :; done\ninput=\"$3\"\noutput=\"$@\"\nfor last; do :; done\ncp \"$INPUT_FILE\" \"$last\"\nprintf 'out_time_ms=1000\\nprogress=continue\\nprogress=end\\n'\n",
        );
        let ffprobe = fake_ffprobe_validation(
            &temp_dir,
            "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"flac\",\"duration\":\"1.0\"},\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"flac\",\"sample_rate\":\"16000\",\"channels\":1}]}\nJSON\n",
        );
        let runtime = WorkerRuntime::new("worker-test".to_string()).with_handler(
            JobType::AudioValidation,
            audio_validation_handler(ffmpeg, ffprobe),
        );
        let created = database
            .jobs()
            .create(
                project_id,
                Some(media_asset_id),
                JobType::AudioValidation,
                0,
                Utc::now(),
            )
            .await
            .expect("job should be created");
        let job = database
            .jobs()
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .expect("job claim should succeed")
            .expect("job should be claimable");
        assert_eq!(job.id, created.id);
        let job_id = job.id;
        let handlers = runtime.handlers.clone();
        std::env::set_var("INPUT_FILE", extraction_tmp.to_string_lossy().to_string());
        runtime
            .execute_job(
                AppState {
                    database: database.clone(),
                    staging_dir: temp_dir.path().to_path_buf(),
                    audio_storage: AudioStorageConfig::new(
                        temp_dir.path().join("audio"),
                        temp_dir.path().join("audio-tmp"),
                    ),
                    audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(
                    ),
                    staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                        1,
                        2,
                        DefaultFilesystemMetrics,
                    )),
                },
                handlers,
                job,
            )
            .await
            .expect("validation should succeed");
        let asset = database
            .audio_assets()
            .get_by_media_asset(media_asset_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.processing_status, "validated");
        assert_eq!(
            asset.normalization_profile.as_deref(),
            Some("flac:flac:16000:1")
        );
        assert_eq!(asset.validation_status.as_deref(), Some("validated"));
        assert!(asset.validated_at.is_some());
        assert_eq!(asset.audio_format, "flac");
        assert_eq!(asset.sample_rate_hz, 16000);
        assert_eq!(asset.channels, 1);
        assert_eq!(
            asset.storage_path,
            temp_dir
                .path()
                .join("audio")
                .join("audio-assets")
                .join(format!("{}.flac", asset.id))
                .to_string_lossy()
                .to_string()
        );
        let media_asset = database
            .media_assets()
            .get(media_asset_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(media_asset.extraction_status.as_deref(), Some("completed"));
        assert_eq!(
            media_asset.extraction_progress.as_deref(),
            Some("validated")
        );
        assert_eq!(media_asset.extraction_tmp_path.as_deref(), None);
        let completed_job = database.jobs().get(job_id).await.unwrap().unwrap();
        assert_eq!(completed_job.status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn worker_runtime_rejects_empty_normalized_audio() {
        clear_env();
        let pool = test_pool().await;
        let temp_dir = TempDir::new().expect("temp dir");
        let (project_id, _source_id, media_asset_id, extraction_tmp) =
            seed_validating_media_asset(&pool, &temp_dir).await;
        let database = Database::from_pool(pool.clone());
        let ffmpeg = fake_ffmpeg(
            &temp_dir,
            "#!/bin/sh\nfor last; do :; done\n: > \"$last\"\n",
        );
        let ffprobe = fake_ffprobe_validation(&temp_dir, "#!/bin/sh\nexit 1\n");
        let job = Job {
            id: Uuid::new_v4(),
            project_id,
            source_id: Some(media_asset_id),
            job_type: JobType::AudioValidation,
            status: JobStatus::Running,
            priority: 0,
            attempts: 0,
            available_at: Utc::now(),
            lease_expires_at: None,
            worker_id: None,
            failure_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        std::env::set_var("INPUT_FILE", extraction_tmp.to_string_lossy().to_string());
        let outcome = handle_audio_validation_job(
            &AppState {
                database: database.clone(),
                staging_dir: temp_dir.path().to_path_buf(),
                audio_storage: AudioStorageConfig::new(
                    temp_dir.path().join("audio"),
                    temp_dir.path().join("audio-tmp"),
                ),
                audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
                staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                    1,
                    2,
                    DefaultFilesystemMetrics,
                )),
            },
            job,
            &ffmpeg,
            &ffprobe,
        )
        .await;
        assert!(outcome.is_ok());
        let asset = database
            .audio_assets()
            .get_by_media_asset(media_asset_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.validation_status.as_deref(), Some("rejected"));
    }

    #[tokio::test]
    async fn audio_completion_rolls_back_when_media_update_fails() {
        clear_env();
        let pool = test_pool().await;
        let temp_dir = TempDir::new().expect("temp dir");
        let (project_id, _source_id, media_asset_id, extraction_tmp) =
            seed_validating_media_asset(&pool, &temp_dir).await;
        let database = Database::from_pool(pool.clone());
        let created = database
            .jobs()
            .create(
                project_id,
                Some(media_asset_id),
                JobType::AudioValidation,
                0,
                Utc::now(),
            )
            .await
            .expect("job should be created");
        let job = database
            .jobs()
            .claim_next("worker-test", Utc::now() + chrono::Duration::minutes(5))
            .await
            .expect("job claim should succeed")
            .expect("job should be claimable");
        assert_eq!(job.id, created.id);
        let media_asset = database
            .media_assets()
            .get(media_asset_id)
            .await
            .unwrap()
            .unwrap();
        sqlx::query("delete from media_assets where id = $1")
            .bind(media_asset_id)
            .execute(&pool)
            .await
            .expect("media asset deletion should succeed");

        let result = complete_audio_validation(
            &AppState {
                database: database.clone(),
                staging_dir: temp_dir.path().to_path_buf(),
                audio_storage: AudioStorageConfig::new(
                    temp_dir.path().join("audio"),
                    temp_dir.path().join("audio-tmp"),
                ),
                audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
                staging_capacity_guard: Arc::new(StagingCapacityGuard::new(
                    1,
                    2,
                    DefaultFilesystemMetrics,
                )),
            },
            &job,
            &media_asset,
            &extraction_tmp,
            "flac:flac:16000:1",
        )
        .await;
        assert!(result.is_err());
        assert!(database
            .audio_assets()
            .get_by_media_asset(media_asset_id)
            .await
            .unwrap()
            .is_none());
        let current_job = database.jobs().get(job.id).await.unwrap().unwrap();
        assert_eq!(current_job.status, JobStatus::Running);
    }

    #[test]
    fn staging_capacity_guard_uses_hysteresis() {
        let metrics = FakeFilesystemMetrics::new(5, 10);
        let guard = StagingCapacityGuard::new(6, 8, metrics.clone());

        let first = guard.check(Path::new("/tmp")).expect("capacity check");
        assert!(first.paused);

        metrics.set_free_bytes(7);
        let second = guard.check(Path::new("/tmp")).expect("capacity check");
        assert!(second.paused);

        metrics.set_free_bytes(9);
        let third = guard.check(Path::new("/tmp")).expect("capacity check");
        assert!(!third.paused);
    }
}

async fn handle_audio_extraction_job(
    state: &AppState,
    job: Job,
    ffmpeg_path: &Path,
) -> Result<JobOutcome> {
    let media_asset_id = job
        .source_id
        .ok_or_else(|| anyhow!("audio extraction job requires a media asset id"))?;
    let media_asset = state
        .database
        .media_assets()
        .get(media_asset_id)
        .await?
        .ok_or_else(|| anyhow!("media asset not found"))?;
    if media_asset.inspection_status.as_deref() != Some("inspectable") {
        return Ok(JobOutcome::Failed {
            failure_reason: "media asset is not inspectable".to_string(),
        });
    }
    let adapter = match FfmpegAdapter::new(ffmpeg_path) {
        Ok(adapter) => adapter,
        Err(err) => {
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
    };
    let temp_path = state
        .staging_dir
        .join(format!("{}.flac.tmp", media_asset_id));
    if let Some(parent) = temp_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let _ = tokio::fs::remove_file(&temp_path).await;
    persist_audio_extraction_state(
        &state.database,
        &media_asset,
        Some("running"),
        Some("starting"),
        Some(temp_path.to_string_lossy().as_ref()),
        None,
    )
    .await?;
    let extraction = adapter
        .start(Path::new(&media_asset.staged_path), &temp_path)
        .await;
    let mut process = match extraction {
        Ok(process) => process,
        Err(err) => {
            let _ = persist_audio_extraction_state(
                &state.database,
                &media_asset,
                Some("failed"),
                Some(&err.to_string()),
                None,
                None,
            )
            .await;
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            });
        }
    };
    let mut last_progress = String::new();
    while let Some(progress) = process.next_progress().await {
        last_progress = progress.raw.clone();
        let temp_path_string = temp_path.to_string_lossy().to_string();
        persist_audio_extraction_state(
            &state.database,
            &media_asset,
            Some("running"),
            Some(&last_progress),
            Some(temp_path_string.as_str()),
            None,
        )
        .await?;
    }
    let status = process.wait().await;
    match status {
        Ok(output) if output.status.success() => {
            persist_audio_extraction_state(
                &state.database,
                &media_asset,
                Some("completed"),
                Some(if last_progress.is_empty() {
                    "completed"
                } else {
                    &last_progress
                }),
                Some(temp_path.to_string_lossy().as_ref()),
                Some(Utc::now()),
            )
            .await?;
            Ok(JobOutcome::Completed)
        }
        Ok(_) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            persist_audio_extraction_state(
                &state.database,
                &media_asset,
                Some("failed"),
                Some("ffmpeg failed"),
                None,
                None,
            )
            .await?;
            Ok(JobOutcome::Retry {
                failure_reason: "ffmpeg failed".to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
        Err(err) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            persist_audio_extraction_state(
                &state.database,
                &media_asset,
                Some("failed"),
                Some(&err.to_string()),
                None,
                None,
            )
            .await?;
            Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            })
        }
    }
}

async fn persist_audio_extraction_state(
    database: &crate::db::Database,
    media_asset: &crate::media_asset::MediaAsset,
    extraction_status: Option<&str>,
    extraction_progress: Option<&str>,
    extraction_tmp_path: Option<&str>,
    extracted_at: Option<chrono::DateTime<Utc>>,
) -> Result<()> {
    let _ = database
        .media_assets()
        .upsert_ingested(
            database.pool(),
            crate::media_asset::MediaAssetUpsert {
                project_id: media_asset.project_id,
                source_id: media_asset.source_id,
                media_kind: media_asset.media_kind.clone(),
                staged_path: media_asset.staged_path.clone(),
                original_filename: media_asset.original_filename.clone(),
                byte_size: media_asset.byte_size,
                ingestion_status: media_asset.ingestion_status.clone(),
                inspection_status: media_asset.inspection_status.clone(),
                extraction_status: extraction_status.map(str::to_string),
                extraction_progress: extraction_progress.map(str::to_string),
                extraction_tmp_path: extraction_tmp_path.map(str::to_string),
                extracted_at,
                container_format: media_asset.container_format.clone(),
                duration_seconds: media_asset.duration_seconds,
                codecs: media_asset.codecs.clone(),
                sample_rate_hz: media_asset.sample_rate_hz,
                channels: media_asset.channels,
                inspected_at: media_asset.inspected_at,
            },
        )
        .await?;
    Ok(())
}

async fn handle_audio_validation_job(
    state: &AppState,
    job: Job,
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
) -> Result<JobOutcome> {
    let media_asset_id = job
        .source_id
        .ok_or_else(|| anyhow!("audio validation job requires a media asset id"))?;
    let media_asset = state
        .database
        .media_assets()
        .get(media_asset_id)
        .await?
        .ok_or_else(|| anyhow!("media asset not found"))?;
    let extraction_tmp_path = media_asset
        .extraction_tmp_path
        .clone()
        .ok_or_else(|| anyhow!("media asset missing extracted temp path"))?;
    let profile = state.audio_normalization_profile.clone();
    let profile_string = profile_label(&profile);
    let adapter = FfmpegAdapter::new(ffmpeg_path)?;
    let ffprobe = FfprobeAdapter::new(ffprobe_path)?;
    let normalized_path = state
        .audio_storage
        .temporary_normalization_path(media_asset_id)?;
    if let Some(parent) = normalized_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let _ = tokio::fs::remove_file(&normalized_path).await;
    persist_audio_validation_state(
        &state.database,
        &media_asset,
        Some("normalizing"),
        Some(profile_string.as_str()),
        None,
        None,
        Some(normalized_path.to_string_lossy().as_ref()),
    )
    .await?;
    let mut process = adapter
        .start(Path::new(&extraction_tmp_path), &normalized_path)
        .await?;
    while let Some(progress) = process.next_progress().await {
        persist_audio_validation_state(
            &state.database,
            &media_asset,
            Some("normalizing"),
            Some(profile_string.as_str()),
            Some(&progress.raw),
            None,
            Some(normalized_path.to_string_lossy().as_ref()),
        )
        .await?;
    }
    let output = match process.wait().await {
        Ok(output) => output,
        Err(err) => {
            let _ = tokio::fs::remove_file(&normalized_path).await;
            persist_audio_validation_state(
                &state.database,
                &media_asset,
                Some("failed"),
                Some(profile_string.as_str()),
                None,
                Some(&err.to_string()),
                None,
            )
            .await?;
            return Ok(JobOutcome::Retry {
                failure_reason: err.to_string(),
                retry_after: Utc::now() + chrono::Duration::minutes(5),
            });
        }
    };
    if !output.status.success() {
        let _ = tokio::fs::remove_file(&normalized_path).await;
        persist_audio_validation_state(
            &state.database,
            &media_asset,
            Some("failed"),
            Some(profile_string.as_str()),
            None,
            Some("ffmpeg failed"),
            None,
        )
        .await?;
        return Ok(JobOutcome::Retry {
            failure_reason: "ffmpeg failed".to_string(),
            retry_after: Utc::now() + chrono::Duration::minutes(5),
        });
    }
    let validation = validate_normalized_audio(&ffprobe, &normalized_path, &profile).await;
    match validation {
        Ok(validated) => {
            if validated {
                complete_audio_validation(
                    state,
                    &job,
                    &media_asset,
                    &normalized_path,
                    &profile_string,
                )
                .await?;
                Ok(JobOutcome::Completed)
            } else {
                Ok(JobOutcome::Failed {
                    failure_reason: "audio validation rejected output".to_string(),
                })
            }
        }
        Err(err) => {
            persist_audio_validation_state(
                &state.database,
                &media_asset,
                Some("rejected"),
                Some(profile_string.as_str()),
                Some("rejected"),
                Some(&err.to_string()),
                Some(normalized_path.to_string_lossy().as_ref()),
            )
            .await?;
            Ok(JobOutcome::Failed {
                failure_reason: err.to_string(),
            })
        }
    }
}

async fn complete_audio_validation(
    state: &AppState,
    job: &Job,
    media_asset: &crate::media_asset::MediaAsset,
    normalized_path: &Path,
    profile_string: &str,
) -> Result<()> {
    let audio_asset_id = state
        .database
        .audio_assets()
        .get_by_media_asset(media_asset.id)
        .await?
        .map(|asset| asset.id)
        .unwrap_or_else(Uuid::new_v4);
    let byte_size = tokio::fs::metadata(normalized_path).await?.len() as i64;
    let final_path =
        state
            .audio_storage
            .finalize_audio_path(normalized_path, audio_asset_id, byte_size)?;
    let mut tx = state.database.pool().begin().await?;
    let audio_asset = state
        .database
        .audio_assets()
        .upsert(
            &mut *tx,
            crate::audio_asset::AudioAssetUpsert {
                id: audio_asset_id,
                media_asset_id: media_asset.id,
                project_id: media_asset.project_id,
                processing_status: "validated".to_string(),
                normalization_profile: Some(profile_string.to_string()),
                validation_status: Some("validated".to_string()),
                validation_error: None,
                validated_at: Some(Utc::now()),
                duration_seconds: media_asset.duration_seconds.unwrap_or(0.0),
                audio_format: "flac".to_string(),
                codec: "flac".to_string(),
                sample_rate_hz: 16_000,
                channels: 1,
                byte_size,
                storage_path: final_path.to_string_lossy().to_string(),
            },
        )
        .await?;
    let media_update = sqlx::query(
        r#"
        update media_assets
        set extraction_status = 'completed',
            extraction_progress = 'validated',
            extraction_tmp_path = null,
            extracted_at = now(),
            container_format = $2,
            duration_seconds = $3,
            codecs = $4,
            sample_rate_hz = $5,
            channels = $6,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(media_asset.id)
    .bind(&audio_asset.audio_format)
    .bind(audio_asset.duration_seconds)
    .bind(&audio_asset.codec)
    .bind(audio_asset.sample_rate_hz)
    .bind(audio_asset.channels)
    .execute(&mut *tx)
    .await?;
    if media_update.rows_affected() != 1 {
        return Err(anyhow!("media asset not found"));
    }
    let job_update = sqlx::query(
        r#"
        update jobs
        set status = 'completed',
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job.id)
    .execute(&mut *tx)
    .await?;
    if job_update.rows_affected() != 1 {
        return Err(anyhow!("job not found"));
    }
    tx.commit().await?;
    Ok(())
}

async fn validate_normalized_audio(
    ffprobe: &FfprobeAdapter,
    path: &Path,
    profile: &AudioNormalizationProfile,
) -> Result<bool> {
    let metadata = tokio::fs::metadata(path).await?;
    if metadata.len() == 0 {
        return Err(anyhow!("audio output is empty"));
    }
    let probed = ffprobe
        .probe(path, std::time::Duration::from_secs(5))
        .await?;
    let format_name = probed
        .format
        .as_ref()
        .and_then(|format| format.format_name.clone())
        .ok_or_else(|| anyhow!("audio output missing container format"))?;
    let duration = probed
        .format
        .as_ref()
        .and_then(|format| format.duration.as_deref())
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or_else(|| anyhow!("audio output missing duration"))?;
    if duration <= 0.0 {
        return Err(anyhow!("audio duration must be positive"));
    }
    let audio_stream = probed
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("audio"))
        .ok_or_else(|| anyhow!("audio output missing audio stream"))?;
    let sample_rate = audio_stream
        .sample_rate
        .as_deref()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| anyhow!("audio output missing sample rate"))?;
    let channels = audio_stream
        .channels
        .ok_or_else(|| anyhow!("audio output missing channels"))?;
    let codec = audio_stream
        .codec_name
        .as_deref()
        .ok_or_else(|| anyhow!("audio output missing codec"))?;
    if format_name != profile.format {
        return Err(anyhow!("audio output does not match normalization format"));
    }
    if codec != profile.codec {
        return Err(anyhow!("audio output does not match normalization codec"));
    }
    if sample_rate != profile.sample_rate_hz {
        return Err(anyhow!(
            "audio output does not match normalization sample rate"
        ));
    }
    if channels != profile.channels {
        return Err(anyhow!(
            "audio output does not match normalization channels"
        ));
    }
    Ok(true)
}

async fn persist_audio_validation_state(
    database: &crate::db::Database,
    media_asset: &crate::media_asset::MediaAsset,
    processing_status: Option<&str>,
    normalization_profile: Option<&str>,
    validation_status: Option<&str>,
    validation_error: Option<&str>,
    storage_path: Option<&str>,
) -> Result<()> {
    let _ = database
        .audio_assets()
        .upsert(
            database.pool(),
            crate::audio_asset::AudioAssetUpsert {
                id: Uuid::new_v4(),
                media_asset_id: media_asset.id,
                project_id: media_asset.project_id,
                processing_status: processing_status.unwrap_or("normalized").to_string(),
                normalization_profile: normalization_profile.map(str::to_string),
                validation_status: validation_status.map(str::to_string),
                validation_error: validation_error.map(str::to_string),
                validated_at: validation_status.map(|_| Utc::now()),
                duration_seconds: media_asset.duration_seconds.unwrap_or(0.0),
                audio_format: "flac".to_string(),
                codec: "flac".to_string(),
                sample_rate_hz: 16_000,
                channels: 1,
                byte_size: if let Some(path) = storage_path {
                    tokio::fs::metadata(path)
                        .await
                        .map(|meta| meta.len() as i64)
                        .unwrap_or(0)
                } else {
                    0
                },
                storage_path: storage_path.unwrap_or_default().to_string(),
            },
        )
        .await?;
    Ok(())
}

fn profile_label(profile: &AudioNormalizationProfile) -> String {
    format!(
        "{}:{}:{}:{}",
        profile.format, profile.codec, profile.sample_rate_hz, profile.channels
    )
}
