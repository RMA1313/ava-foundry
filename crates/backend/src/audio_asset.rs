use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioAsset {
    pub id: Uuid,
    pub media_asset_id: Uuid,
    pub project_id: Uuid,
    pub processing_status: String,
    pub duration_seconds: f64,
    pub audio_format: String,
    pub codec: String,
    pub sample_rate_hz: i32,
    pub channels: i32,
    pub byte_size: i64,
    pub storage_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioAssetUpsert {
    pub media_asset_id: Uuid,
    pub project_id: Uuid,
    pub processing_status: String,
    pub duration_seconds: f64,
    pub audio_format: String,
    pub codec: String,
    pub sample_rate_hz: i32,
    pub channels: i32,
    pub byte_size: i64,
    pub storage_path: String,
}

#[derive(Debug, Clone)]
pub struct AudioAssetRepository {
    pool: PgPool,
}

impl AudioAssetRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert<'e, E>(&self, executor: E, request: AudioAssetUpsert) -> Result<AudioAsset>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into audio_assets (
                id, media_asset_id, project_id, processing_status, duration_seconds,
                audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now())
            on conflict (media_asset_id)
            do update set
                project_id = excluded.project_id,
                processing_status = excluded.processing_status,
                duration_seconds = excluded.duration_seconds,
                audio_format = excluded.audio_format,
                codec = excluded.codec,
                sample_rate_hz = excluded.sample_rate_hz,
                channels = excluded.channels,
                byte_size = excluded.byte_size,
                storage_path = excluded.storage_path,
                updated_at = excluded.updated_at
            returning id, media_asset_id, project_id, processing_status, duration_seconds,
                      audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(request.media_asset_id)
        .bind(request.project_id)
        .bind(request.processing_status.as_str())
        .bind(request.duration_seconds)
        .bind(request.audio_format.as_str())
        .bind(request.codec.as_str())
        .bind(request.sample_rate_hz)
        .bind(request.channels)
        .bind(request.byte_size)
        .bind(request.storage_path.as_str())
        .fetch_one(executor)
        .await?;
        audio_asset_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<AudioAsset>> {
        let row = sqlx::query(
            r#"
            select id, media_asset_id, project_id, processing_status, duration_seconds,
                   audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                   created_at, updated_at
            from audio_assets
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(audio_asset_from_row).transpose()
    }

    pub async fn list(&self, project_id: Uuid) -> Result<Vec<AudioAsset>> {
        let rows = sqlx::query(
            r#"
            select id, media_asset_id, project_id, processing_status, duration_seconds,
                   audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                   created_at, updated_at
            from audio_assets
            where project_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(audio_asset_from_row).collect()
    }
}

fn audio_asset_from_row(row: &PgRow) -> Result<AudioAsset> {
    Ok(AudioAsset {
        id: row.try_get("id")?,
        media_asset_id: row.try_get("media_asset_id")?,
        project_id: row.try_get("project_id")?,
        processing_status: row.try_get("processing_status")?,
        duration_seconds: row.try_get("duration_seconds")?,
        audio_format: row.try_get("audio_format")?,
        codec: row.try_get("codec")?,
        sample_rate_hz: row.try_get("sample_rate_hz")?,
        channels: row.try_get("channels")?,
        byte_size: row.try_get("byte_size")?,
        storage_path: row.try_get("storage_path")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
