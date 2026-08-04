use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct MediaAsset {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_id: Uuid,
    pub media_kind: String,
    pub staged_path: String,
    pub original_filename: Option<String>,
    pub byte_size: i64,
    pub ingestion_status: String,
    pub inspection_status: Option<String>,
    pub container_format: Option<String>,
    pub duration_seconds: Option<f64>,
    pub codecs: Option<String>,
    pub sample_rate_hz: Option<i32>,
    pub channels: Option<i32>,
    pub inspected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaAssetUpsert {
    pub project_id: Uuid,
    pub source_id: Uuid,
    pub media_kind: String,
    pub staged_path: String,
    pub original_filename: Option<String>,
    pub byte_size: i64,
    pub ingestion_status: String,
    pub inspection_status: Option<String>,
    pub container_format: Option<String>,
    pub duration_seconds: Option<f64>,
    pub codecs: Option<String>,
    pub sample_rate_hz: Option<i32>,
    pub channels: Option<i32>,
    pub inspected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct MediaAssetRepository {
    pool: PgPool,
}

impl MediaAssetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_ingested<'e, E>(
        &self,
        executor: E,
        request: MediaAssetUpsert,
    ) -> Result<MediaAsset>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into media_assets (
                id, project_id, source_id, media_kind, staged_path,
                original_filename, byte_size, ingestion_status,
                inspection_status, container_format, duration_seconds, codecs,
                sample_rate_hz, channels, inspected_at, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, now(), now())
            on conflict (source_id)
            do update set
                project_id = excluded.project_id,
                media_kind = excluded.media_kind,
                staged_path = excluded.staged_path,
                original_filename = excluded.original_filename,
                byte_size = excluded.byte_size,
                ingestion_status = excluded.ingestion_status,
                inspection_status = excluded.inspection_status,
                container_format = excluded.container_format,
                duration_seconds = excluded.duration_seconds,
                codecs = excluded.codecs,
                sample_rate_hz = excluded.sample_rate_hz,
                channels = excluded.channels,
                inspected_at = excluded.inspected_at,
                updated_at = excluded.updated_at
            returning id, project_id, source_id, media_kind, staged_path,
                      original_filename, byte_size, ingestion_status,
                      inspection_status, container_format, duration_seconds, codecs,
                      sample_rate_hz, channels, inspected_at,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(request.project_id)
        .bind(request.source_id)
        .bind(request.media_kind.as_str())
        .bind(request.staged_path.as_str())
        .bind(request.original_filename.as_deref())
        .bind(request.byte_size)
        .bind(request.ingestion_status.as_str())
        .bind(request.inspection_status.as_deref())
        .bind(request.container_format.as_deref())
        .bind(request.duration_seconds)
        .bind(request.codecs.as_deref())
        .bind(request.sample_rate_hz)
        .bind(request.channels)
        .bind(request.inspected_at)
        .fetch_one(executor)
        .await?;
        media_asset_from_row(&row)
    }

    pub async fn get_by_source(&self, source_id: Uuid) -> Result<Option<MediaAsset>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_id, media_kind, staged_path,
                   original_filename, byte_size, ingestion_status,
                   inspection_status, container_format, duration_seconds, codecs,
                   sample_rate_hz, channels, inspected_at,
                   created_at, updated_at
            from media_assets
            where source_id = $1
            "#,
        )
        .bind(source_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(media_asset_from_row).transpose()
    }
}

fn media_asset_from_row(row: &PgRow) -> Result<MediaAsset> {
    Ok(MediaAsset {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        source_id: row.try_get("source_id")?,
        media_kind: row.try_get("media_kind")?,
        staged_path: row.try_get("staged_path")?,
        original_filename: row.try_get("original_filename")?,
        byte_size: row.try_get("byte_size")?,
        ingestion_status: row.try_get("ingestion_status")?,
        inspection_status: row.try_get("inspection_status")?,
        container_format: row.try_get("container_format")?,
        duration_seconds: row.try_get("duration_seconds")?,
        codecs: row.try_get("codecs")?,
        sample_rate_hz: row.try_get("sample_rate_hz")?,
        channels: row.try_get("channels")?,
        inspected_at: row.try_get("inspected_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
