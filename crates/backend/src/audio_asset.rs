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
    pub normalization_profile: Option<String>,
    pub validation_status: Option<String>,
    pub validation_error: Option<String>,
    pub validated_at: Option<DateTime<Utc>>,
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
    pub id: Uuid,
    pub media_asset_id: Uuid,
    pub project_id: Uuid,
    pub processing_status: String,
    pub normalization_profile: Option<String>,
    pub validation_status: Option<String>,
    pub validation_error: Option<String>,
    pub validated_at: Option<DateTime<Utc>>,
    pub duration_seconds: f64,
    pub audio_format: String,
    pub codec: String,
    pub sample_rate_hz: i32,
    pub channels: i32,
    pub byte_size: i64,
    pub storage_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioAssetListFilter {
    pub project_id: Uuid,
    pub processing_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioAssetPage {
    pub items: Vec<AudioAsset>,
    pub limit: u32,
    pub offset: u32,
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
        let row = sqlx::query(
            r#"
            insert into audio_assets (
                id, media_asset_id, project_id, processing_status, duration_seconds,
                normalization_profile, validation_status, validation_error, validated_at,
                audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, now(), now())
            on conflict (media_asset_id)
            do update set
                project_id = excluded.project_id,
                processing_status = excluded.processing_status,
                duration_seconds = excluded.duration_seconds,
                normalization_profile = excluded.normalization_profile,
                validation_status = excluded.validation_status,
                validation_error = excluded.validation_error,
                validated_at = excluded.validated_at,
                audio_format = excluded.audio_format,
                codec = excluded.codec,
                sample_rate_hz = excluded.sample_rate_hz,
                channels = excluded.channels,
                byte_size = excluded.byte_size,
                storage_path = excluded.storage_path,
                updated_at = excluded.updated_at
            returning id, media_asset_id, project_id, processing_status, duration_seconds,
                      normalization_profile, validation_status, validation_error, validated_at,
                      audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                      created_at, updated_at
            "#,
        )
        .bind(request.id)
        .bind(request.media_asset_id)
        .bind(request.project_id)
        .bind(request.processing_status.as_str())
        .bind(request.duration_seconds)
        .bind(request.normalization_profile.as_deref())
        .bind(request.validation_status.as_deref())
        .bind(request.validation_error.as_deref())
        .bind(request.validated_at)
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
                   normalization_profile, validation_status, validation_error, validated_at,
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
        self.list_paginated(
            AudioAssetListFilter {
                project_id,
                processing_status: None,
            },
            100,
            0,
        )
        .await
        .map(|page| page.items)
    }

    pub async fn list_paginated(
        &self,
        filter: AudioAssetListFilter,
        limit: i64,
        offset: i64,
    ) -> Result<AudioAssetPage> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let rows = sqlx::query(
            r#"
            select id, media_asset_id, project_id, processing_status, duration_seconds,
                   normalization_profile, validation_status, validation_error, validated_at,
                   audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                   created_at, updated_at
            from audio_assets
            where project_id = $1
              and ($2::text is null or processing_status = $2)
            order by created_at asc, id asc
            limit $3 offset $4
            "#,
        )
        .bind(filter.project_id)
        .bind(filter.processing_status.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let items = rows
            .iter()
            .map(audio_asset_from_row)
            .collect::<Result<Vec<_>>>()?;
        Ok(AudioAssetPage {
            items,
            limit: limit as u32,
            offset: offset as u32,
        })
    }

    pub async fn get_by_media_asset(&self, media_asset_id: Uuid) -> Result<Option<AudioAsset>> {
        let row = sqlx::query(
            r#"
            select id, media_asset_id, project_id, processing_status, duration_seconds,
                   normalization_profile, validation_status, validation_error, validated_at,
                   audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                   created_at, updated_at
            from audio_assets
            where media_asset_id = $1
            "#,
        )
        .bind(media_asset_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(audio_asset_from_row).transpose()
    }

    pub async fn get_in_project(&self, project_id: Uuid, id: Uuid) -> Result<Option<AudioAsset>> {
        let row = sqlx::query(
            r#"
            select id, media_asset_id, project_id, processing_status, duration_seconds,
                   normalization_profile, validation_status, validation_error, validated_at,
                   audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
                   created_at, updated_at
            from audio_assets
            where project_id = $1
              and id = $2
            "#,
        )
        .bind(project_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(audio_asset_from_row).transpose()
    }
}

fn audio_asset_from_row(row: &PgRow) -> Result<AudioAsset> {
    Ok(AudioAsset {
        id: row.try_get("id")?,
        media_asset_id: row.try_get("media_asset_id")?,
        project_id: row.try_get("project_id")?,
        processing_status: row.try_get("processing_status")?,
        duration_seconds: row.try_get("duration_seconds")?,
        normalization_profile: row.try_get("normalization_profile")?,
        validation_status: row.try_get("validation_status")?,
        validation_error: row.try_get("validation_error")?,
        validated_at: row.try_get("validated_at")?,
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
