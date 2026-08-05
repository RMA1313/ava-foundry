use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobType {
    SourceRegistration,
    MediaIngestion,
    MediaInspection,
    AudioExtraction,
    AudioValidation,
    VideoCleanup,
    DuplicateDetection,
    AiAnalysis,
    IntervalMining,
    ManualReview,
    DatasetExport,
}

impl JobType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SourceRegistration => "source_registration",
            Self::MediaIngestion => "media_ingestion",
            Self::MediaInspection => "media_inspection",
            Self::AudioExtraction => "audio_extraction",
            Self::AudioValidation => "audio_validation",
            Self::VideoCleanup => "video_cleanup",
            Self::DuplicateDetection => "duplicate_detection",
            Self::AiAnalysis => "ai_analysis",
            Self::IntervalMining => "interval_mining",
            Self::ManualReview => "manual_review",
            Self::DatasetExport => "dataset_export",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "source_registration" => Ok(Self::SourceRegistration),
            "media_ingestion" => Ok(Self::MediaIngestion),
            "media_inspection" => Ok(Self::MediaInspection),
            "audio_extraction" => Ok(Self::AudioExtraction),
            "audio_validation" => Ok(Self::AudioValidation),
            "video_cleanup" => Ok(Self::VideoCleanup),
            "duplicate_detection" => Ok(Self::DuplicateDetection),
            "ai_analysis" => Ok(Self::AiAnalysis),
            "interval_mining" => Ok(Self::IntervalMining),
            "manual_review" => Ok(Self::ManualReview),
            "dataset_export" => Ok(Self::DatasetExport),
            other => Err(anyhow!("invalid job type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(anyhow!("invalid job status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_id: Option<Uuid>,
    pub parent_job_id: Option<Uuid>,
    pub batch_id: Option<Uuid>,
    pub job_type: JobType,
    pub status: JobStatus,
    pub priority: i32,
    pub attempts: i32,
    pub max_attempts: i32,
    pub retry_count: i32,
    pub available_at: DateTime<Utc>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub worker_id: Option<String>,
    pub progress_current: i64,
    pub progress_total: i64,
    pub result_payload: Option<Value>,
    pub failure_reason: Option<String>,
    pub failure_payload: Option<Value>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct JobRepository {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobListFilter {
    pub project_id: Option<Uuid>,
    pub job_type: Option<JobType>,
    pub status: Option<JobStatus>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobPage {
    pub items: Vec<Job>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobQueueCount {
    pub queued: i64,
    pub running: i64,
}

const JOB_COLUMNS: &str = r#"
    id, project_id, source_id, parent_job_id, batch_id, job_type, status,
    priority, attempts, max_attempts, retry_count, available_at,
    lease_expires_at, worker_id, progress_current, progress_total,
    result_payload, failure_reason, failure_payload,
    cancel_requested_at, cancelled_at, created_at, updated_at
"#;

impl JobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        project_id: Uuid,
        source_id: Option<Uuid>,
        job_type: JobType,
        priority: i32,
        available_at: DateTime<Utc>,
    ) -> Result<Job> {
        validate_priority(priority)?;
        let id = Uuid::new_v4();

        let row = sqlx::query(&format!(
            r#"
            insert into jobs (
                id, project_id, source_id, parent_job_id, batch_id, job_type, status,
                priority, attempts, max_attempts, retry_count, available_at, lease_expires_at,
                worker_id, progress_current, progress_total, result_payload, failure_reason,
                failure_payload, cancel_requested_at, cancelled_at, created_at, updated_at
            )
            values ($1, $2, $3, null, null, $4, $5, $6, 0, 3, 0, $7, null, null, 0, 0, null, null,
                    null, null, null, now(), now())
            on conflict (project_id, source_id, job_type)
            do update set updated_at = excluded.updated_at
            returning {}
            "#,
            JOB_COLUMNS
        ))
        .bind(id)
        .bind(project_id)
        .bind(source_id)
        .bind(job_type.as_str())
        .bind(JobStatus::Queued.as_str())
        .bind(priority)
        .bind(available_at)
        .fetch_one(&self.pool)
        .await?;

        job_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Job>> {
        let row = sqlx::query(&format!(
            r#"
            select {}
            from jobs
            where id = $1
            "#,
            JOB_COLUMNS
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(job_from_row).transpose()
    }

    pub async fn list(&self) -> Result<Vec<Job>> {
        self.list_paginated(None, 100, 0)
            .await
            .map(|page| page.items)
    }

    pub async fn list_paginated(
        &self,
        filter: Option<JobListFilter>,
        limit: i64,
        offset: i64,
    ) -> Result<JobPage> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let project_id = filter.and_then(|filter| filter.project_id);
        let job_type = filter
            .and_then(|filter| filter.job_type)
            .map(|value| value.as_str());
        let status = filter
            .and_then(|filter| filter.status)
            .map(|value| value.as_str());
        let rows = sqlx::query(&format!(
            r#"
            select {}
            from jobs
            where ($1::uuid is null or project_id = $1)
              and ($2::text is null or job_type = $2)
              and ($3::text is null or status = $3)
            order by priority desc, available_at asc, created_at asc, id asc
            limit $4 offset $5
            "#,
            JOB_COLUMNS
        ))
        .bind(project_id)
        .bind(job_type)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let items = rows.iter().map(job_from_row).collect::<Result<Vec<_>>>()?;
        Ok(JobPage {
            items,
            limit: limit as u32,
            offset: offset as u32,
        })
    }

    pub async fn get_by_logical_key(
        &self,
        project_id: Uuid,
        source_id: Option<Uuid>,
        job_type: JobType,
    ) -> Result<Option<Job>> {
        let row = sqlx::query(&format!(
            r#"
            select {}
            from jobs
            where project_id = $1
              and source_id is not distinct from $2
              and job_type = $3
            "#,
            JOB_COLUMNS
        ))
        .bind(project_id)
        .bind(source_id)
        .bind(job_type.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(job_from_row).transpose()
    }

    pub async fn claim_next(
        &self,
        worker_id: &str,
        lease_expires_at: DateTime<Utc>,
    ) -> Result<Option<Job>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(&format!(
            r#"
            with candidate as (
                select id
                from jobs
                where status = 'queued'
                  and available_at <= now()
                order by priority desc, available_at asc, created_at asc, id asc
                for update skip locked
                limit 1
            )
            update jobs
            set status = 'running',
                worker_id = $1,
                lease_expires_at = $2,
                updated_at = now()
            where id in (select id from candidate)
            returning {}
            "#,
            JOB_COLUMNS
        ))
        .bind(worker_id)
        .bind(lease_expires_at)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        row.as_ref().map(job_from_row).transpose()
    }

    pub async fn queue_count(&self, job_type: JobType, project_id: Option<Uuid>) -> Result<i64> {
        let row = sqlx::query(
            r#"
            select count(*)::bigint as count
            from jobs
            where job_type = $1
              and status = 'queued'
              and ($2::uuid is null or project_id = $2)
            "#,
        )
        .bind(job_type.as_str())
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.try_get("count")?)
    }

    pub async fn complete(&self, id: Uuid) -> Result<Option<Job>> {
        self.transition_terminal(id, JobStatus::Completed, None)
            .await
    }

    pub async fn fail(&self, id: Uuid, failure_reason: &str) -> Result<Option<Job>> {
        self.transition_terminal(id, JobStatus::Failed, Some(failure_reason))
            .await
    }

    pub async fn schedule_retry(
        &self,
        id: Uuid,
        failure_reason: &str,
        available_at: DateTime<Utc>,
        max_attempts: i32,
    ) -> Result<Option<Job>> {
        if max_attempts < 1 {
            return Err(anyhow!("max attempts must be at least 1"));
        }

        let mut tx = self.pool.begin().await?;
        let current = sqlx::query(&format!(
            r#"
            select {}
            from jobs
            where id = $1
            for update
            "#,
            JOB_COLUMNS
        ))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(current) = current else {
            tx.commit().await?;
            return Ok(None);
        };
        let job = job_from_row(&current)?;
        if job.status == JobStatus::Queued
            && job.attempts > 0
            && job.worker_id.is_none()
            && job.lease_expires_at.is_none()
        {
            tx.commit().await?;
            return Ok(Some(job));
        }
        if job.status != JobStatus::Failed {
            tx.commit().await?;
            return Err(anyhow!("job must be failed"));
        }
        if job.attempts + 1 >= max_attempts {
            let row = sqlx::query(&format!(
                r#"
                update jobs
                set status = 'failed',
                    attempts = attempts + 1,
                    retry_count = retry_count + 1,
                    failure_reason = $2,
                    lease_expires_at = null,
                    worker_id = null,
                    progress_current = 0,
                    progress_total = 0,
                    updated_at = now()
                where id = $1
                returning {}
                "#,
                JOB_COLUMNS
            ))
            .bind(id)
            .bind(failure_reason)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            return job_from_row(&row).map(Some);
        }

        let row = sqlx::query(&format!(
            r#"
            update jobs
            set status = 'queued',
                attempts = attempts + 1,
                retry_count = retry_count + 1,
                available_at = $2,
                failure_reason = $3,
                lease_expires_at = null,
                worker_id = null,
            progress_current = 0,
            progress_total = 0,
            updated_at = now()
            where id = $1
            returning {}
            "#,
            JOB_COLUMNS
        ))
        .bind(id)
        .bind(available_at)
        .bind(failure_reason)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        job_from_row(&row).map(Some)
    }

    pub async fn recover_expired_leases(&self, now: DateTime<Utc>) -> Result<Vec<Job>> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(&format!(
            r#"
            update jobs
            set status = 'queued',
                lease_expires_at = null,
                worker_id = null,
                updated_at = now()
            where id in (
                select id
                from jobs
                where status = 'running'
                and lease_expires_at is not null
                and lease_expires_at < $1
                for update skip locked
            )
            returning {}
            "#,
            JOB_COLUMNS
        ))
        .bind(now)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.iter().map(job_from_row).collect()
    }

    async fn transition_terminal(
        &self,
        id: Uuid,
        status: JobStatus,
        failure_reason: Option<&str>,
    ) -> Result<Option<Job>> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query(&format!(
            r#"
            select {}
            from jobs
            where id = $1
            for update
            "#,
            JOB_COLUMNS
        ))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            tx.commit().await?;
            return Ok(None);
        };
        let job = job_from_row(&current)?;
        if job.status == status {
            tx.commit().await?;
            return Ok(Some(job));
        }
        if matches!(
            job.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) {
            tx.commit().await?;
            return Ok(Some(job));
        }
        if job.status != JobStatus::Running {
            return Err(anyhow!("job must be running"));
        }

        let row = sqlx::query(&format!(
            r#"
            update jobs
            set status = $2,
                failure_reason = $3,
                lease_expires_at = null,
                worker_id = null,
                updated_at = now()
            where id = $1
            returning {}
            "#,
            JOB_COLUMNS
        ))
        .bind(id)
        .bind(status.as_str())
        .bind(failure_reason)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        job_from_row(&row).map(Some)
    }
}

fn validate_priority(priority: i32) -> Result<()> {
    if !(-1000..=1000).contains(&priority) {
        return Err(anyhow!("job priority must be between -1000 and 1000"));
    }
    Ok(())
}

pub(crate) fn job_from_row(row: &PgRow) -> Result<Job> {
    Ok(Job {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        source_id: row.try_get("source_id")?,
        parent_job_id: row.try_get("parent_job_id")?,
        batch_id: row.try_get("batch_id")?,
        job_type: JobType::parse(row.try_get::<String, _>("job_type")?.as_str())?,
        status: JobStatus::parse(row.try_get::<String, _>("status")?.as_str())?,
        priority: row.try_get("priority")?,
        attempts: row.try_get("attempts")?,
        max_attempts: row.try_get("max_attempts")?,
        retry_count: row.try_get("retry_count")?,
        available_at: row.try_get("available_at")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        worker_id: row.try_get("worker_id")?,
        progress_current: row.try_get("progress_current")?,
        progress_total: row.try_get("progress_total")?,
        result_payload: row.try_get("result_payload")?,
        failure_reason: row.try_get("failure_reason")?,
        failure_payload: row.try_get("failure_payload")?,
        cancel_requested_at: row.try_get("cancel_requested_at")?,
        cancelled_at: row.try_get("cancelled_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRegistration {
    pub id: Uuid,
    pub worker_id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub active_lease_count: i32,
    pub last_heartbeat_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerRepository {
    pool: PgPool,
}

impl WorkerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn register(
        &self,
        worker_id: &str,
        version: &str,
        capabilities: &[JobType],
        expires_at: DateTime<Utc>,
    ) -> Result<WorkerRegistration> {
        let capabilities = capabilities
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>();
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into worker_registry (
                id, worker_id, version, capabilities, active_lease_count,
                last_heartbeat_at, expires_at, created_at, updated_at
            )
            values ($1, $2, $3, $4, 0, now(), $5, now(), now())
            on conflict (worker_id)
            do update set
                version = excluded.version,
                capabilities = excluded.capabilities,
                expires_at = excluded.expires_at,
                last_heartbeat_at = now(),
                updated_at = now()
            returning id, worker_id, version, capabilities, active_lease_count,
                      last_heartbeat_at, expires_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(worker_id)
        .bind(version)
        .bind(&capabilities)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(WorkerRegistration {
            id: row.try_get("id")?,
            worker_id: row.try_get("worker_id")?,
            version: row.try_get("version")?,
            capabilities: row.try_get("capabilities")?,
            active_lease_count: row.try_get("active_lease_count")?,
            last_heartbeat_at: row.try_get("last_heartbeat_at")?,
            expires_at: row.try_get("expires_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    pub async fn heartbeat(
        &self,
        worker_id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Option<WorkerRegistration>> {
        let row = sqlx::query(
            r#"
            update worker_registry
            set last_heartbeat_at = now(),
                expires_at = $2,
                updated_at = now()
            where worker_id = $1
            returning id, worker_id, version, capabilities, active_lease_count,
                      last_heartbeat_at, expires_at, created_at, updated_at
            "#,
        )
        .bind(worker_id)
        .bind(expires_at)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => Ok(Some(WorkerRegistration {
                id: row.try_get("id")?,
                worker_id: row.try_get("worker_id")?,
                version: row.try_get("version")?,
                capabilities: row.try_get("capabilities")?,
                active_lease_count: row.try_get("active_lease_count")?,
                last_heartbeat_at: row.try_get("last_heartbeat_at")?,
                expires_at: row.try_get("expires_at")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_priority, JobStatus, JobType};

    #[test]
    fn job_types_round_trip() {
        assert_eq!(
            JobType::parse("audio_validation").unwrap(),
            JobType::AudioValidation
        );
        assert!(JobType::parse("unknown").is_err());
    }

    #[test]
    fn job_statuses_round_trip() {
        assert_eq!(JobStatus::parse("queued").unwrap(), JobStatus::Queued);
        assert!(JobStatus::parse("bogus").is_err());
    }

    #[test]
    fn job_priority_is_bounded() {
        assert!(validate_priority(0).is_ok());
        assert!(validate_priority(1001).is_err());
    }
}
