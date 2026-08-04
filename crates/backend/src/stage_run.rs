use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct StageRun {
    pub id: Uuid,
    pub project_id: Uuid,
    pub media_asset_id: Option<Uuid>,
    pub audio_asset_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub stage_name: String,
    pub status: String,
    pub queue_wait_ms: i64,
    pub wall_time_ms: i64,
    pub input_bytes: i64,
    pub output_bytes: i64,
    pub input_duration_seconds: f64,
    pub throughput: Option<f64>,
    pub failure_reason: Option<String>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StageRunRepository {
    pool: PgPool,
}

impl StageRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert<'e, E>(&self, executor: E, run: StageRun) -> Result<StageRun>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"
            insert into stage_runs (
                id, project_id, media_asset_id, audio_asset_id, job_id, stage_name,
                status, queue_wait_ms, wall_time_ms, input_bytes, output_bytes,
                input_duration_seconds, throughput, failure_reason, cpu_percent,
                memory_bytes, created_at, updated_at
            )
            values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,now(),now())
            returning id, project_id, media_asset_id, audio_asset_id, job_id, stage_name,
                      status, queue_wait_ms, wall_time_ms, input_bytes, output_bytes,
                      input_duration_seconds, throughput, failure_reason, cpu_percent,
                      memory_bytes, created_at, updated_at
            "#,
        )
        .bind(run.id)
        .bind(run.project_id)
        .bind(run.media_asset_id)
        .bind(run.audio_asset_id)
        .bind(run.job_id)
        .bind(run.stage_name)
        .bind(run.status)
        .bind(run.queue_wait_ms)
        .bind(run.wall_time_ms)
        .bind(run.input_bytes)
        .bind(run.output_bytes)
        .bind(run.input_duration_seconds)
        .bind(run.throughput)
        .bind(run.failure_reason)
        .bind(run.cpu_percent)
        .bind(run.memory_bytes)
        .fetch_one(executor)
        .await?;
        stage_run_from_row(&row)
    }

    pub async fn list_by_project(&self, project_id: Uuid) -> Result<Vec<StageRun>> {
        let rows = sqlx::query(
            r#"
            select id, project_id, media_asset_id, audio_asset_id, job_id, stage_name,
                   status, queue_wait_ms, wall_time_ms, input_bytes, output_bytes,
                   input_duration_seconds, throughput, failure_reason, cpu_percent,
                   memory_bytes, created_at, updated_at
            from stage_runs
            where project_id = $1
            order by created_at desc, id desc
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(stage_run_from_row).collect()
    }
}

fn stage_run_from_row(row: &PgRow) -> Result<StageRun> {
    Ok(StageRun {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        media_asset_id: row.try_get("media_asset_id")?,
        audio_asset_id: row.try_get("audio_asset_id")?,
        job_id: row.try_get("job_id")?,
        stage_name: row.try_get("stage_name")?,
        status: row.try_get("status")?,
        queue_wait_ms: row.try_get("queue_wait_ms")?,
        wall_time_ms: row.try_get("wall_time_ms")?,
        input_bytes: row.try_get("input_bytes")?,
        output_bytes: row.try_get("output_bytes")?,
        input_duration_seconds: row.try_get("input_duration_seconds")?,
        throughput: row.try_get("throughput")?,
        failure_reason: row.try_get("failure_reason")?,
        cpu_percent: row.try_get("cpu_percent")?,
        memory_bytes: row.try_get("memory_bytes")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
