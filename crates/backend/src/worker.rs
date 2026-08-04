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

use crate::{
    download::DownloadCommandAdapter,
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
                if job.job_type != JobType::MediaIngestion {
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
    };
    use chrono::Utc;
    use sqlx::PgPool;
    use std::env;
    use tokio::sync::Mutex;
    use uuid::Uuid;

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
            .execute_job(AppState { database }, handlers, job.clone())
            .await
            .expect("dispatch should succeed");
        assert_eq!(*calls.lock().await, 1);
    }
}
