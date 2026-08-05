use anyhow::{anyhow, Result};
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
    pub extraction_status: Option<String>,
    pub extraction_progress: Option<String>,
    pub extraction_tmp_path: Option<String>,
    pub extracted_at: Option<DateTime<Utc>>,
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
    pub extraction_status: Option<String>,
    pub extraction_progress: Option<String>,
    pub extraction_tmp_path: Option<String>,
    pub extracted_at: Option<DateTime<Utc>>,
    pub container_format: Option<String>,
    pub duration_seconds: Option<f64>,
    pub codecs: Option<String>,
    pub sample_rate_hz: Option<i32>,
    pub channels: Option<i32>,
    pub inspected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAssetListFilter {
    pub project_id: Uuid,
    pub source_id: Option<Uuid>,
    pub ingestion_status: Option<String>,
    pub inspection_status: Option<String>,
    pub extraction_status: Option<String>,
    pub media_kind: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaAssetSortField {
    CreatedAt,
    UpdatedAt,
    MediaKind,
    ByteSize,
}

impl MediaAssetSortField {
    fn as_sql(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::MediaKind => "media_kind",
            Self::ByteSize => "byte_size",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "created_at" => Ok(Self::CreatedAt),
            "updated_at" => Ok(Self::UpdatedAt),
            "media_kind" => Ok(Self::MediaKind),
            "byte_size" => Ok(Self::ByteSize),
            other => Err(anyhow!("invalid media asset sort field: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaAssetListSort {
    pub field: MediaAssetSortField,
    pub descending: bool,
}

impl Default for MediaAssetListSort {
    fn default() -> Self {
        Self {
            field: MediaAssetSortField::CreatedAt,
            descending: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaAssetPage {
    pub items: Vec<MediaAsset>,
    pub limit: u32,
    pub offset: u32,
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
                inspection_status, extraction_status, extraction_progress,
                extraction_tmp_path, extracted_at, container_format,
                duration_seconds, codecs, sample_rate_hz, channels, inspected_at,
                created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, now(), now())
            on conflict (source_id)
            do update set
                project_id = excluded.project_id,
                media_kind = excluded.media_kind,
                staged_path = excluded.staged_path,
                original_filename = excluded.original_filename,
                byte_size = excluded.byte_size,
                ingestion_status = excluded.ingestion_status,
                inspection_status = excluded.inspection_status,
                extraction_status = excluded.extraction_status,
                extraction_progress = excluded.extraction_progress,
                extraction_tmp_path = excluded.extraction_tmp_path,
                extracted_at = excluded.extracted_at,
                container_format = excluded.container_format,
                duration_seconds = excluded.duration_seconds,
                codecs = excluded.codecs,
                sample_rate_hz = excluded.sample_rate_hz,
                channels = excluded.channels,
                inspected_at = excluded.inspected_at,
                updated_at = excluded.updated_at
            returning id, project_id, source_id, media_kind, staged_path,
                      original_filename, byte_size, ingestion_status,
                      inspection_status, extraction_status, extraction_progress,
                      extraction_tmp_path, extracted_at, container_format,
                      duration_seconds, codecs, sample_rate_hz, channels, inspected_at,
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
        .bind(request.extraction_status.as_deref())
        .bind(request.extraction_progress.as_deref())
        .bind(request.extraction_tmp_path.as_deref())
        .bind(request.extracted_at)
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
                   inspection_status, extraction_status, extraction_progress,
                   extraction_tmp_path, extracted_at, container_format,
                   duration_seconds, codecs, sample_rate_hz, channels, inspected_at,
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

    pub async fn get(&self, id: Uuid) -> Result<Option<MediaAsset>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_id, media_kind, staged_path,
                   original_filename, byte_size, ingestion_status,
                   inspection_status, extraction_status, extraction_progress,
                   extraction_tmp_path, extracted_at, container_format,
                   duration_seconds, codecs, sample_rate_hz, channels, inspected_at,
                   created_at, updated_at
            from media_assets
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(media_asset_from_row).transpose()
    }

    pub async fn get_in_project(&self, project_id: Uuid, id: Uuid) -> Result<Option<MediaAsset>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_id, media_kind, staged_path,
                   original_filename, byte_size, ingestion_status,
                   inspection_status, extraction_status, extraction_progress,
                   extraction_tmp_path, extracted_at, container_format,
                   duration_seconds, codecs, sample_rate_hz, channels, inspected_at,
                   created_at, updated_at
            from media_assets
            where project_id = $1 and id = $2
            "#,
        )
        .bind(project_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(media_asset_from_row).transpose()
    }

    pub async fn list_paginated(
        &self,
        filter: MediaAssetListFilter,
        sort: Option<MediaAssetListSort>,
        limit: i64,
        offset: i64,
    ) -> Result<MediaAssetPage> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let sort = sort.unwrap_or_default();
        let order_by = format!(
            "{} {}, id {}",
            sort.field.as_sql(),
            if sort.descending { "desc" } else { "asc" },
            if sort.descending { "desc" } else { "asc" }
        );
        let rows = sqlx::query(&format!(
            r#"
            select id, project_id, source_id, media_kind, staged_path,
                   original_filename, byte_size, ingestion_status,
                   inspection_status, extraction_status, extraction_progress,
                   extraction_tmp_path, extracted_at, container_format,
                   duration_seconds, codecs, sample_rate_hz, channels, inspected_at,
                    created_at, updated_at
            from media_assets
            where project_id = $1
              and ($2::uuid is null or source_id = $2)
              and ($3::text is null or ingestion_status = $3)
              and ($4::text is null or inspection_status = $4)
              and ($5::text is null or extraction_status = $5)
              and ($6::text is null or media_kind = $6)
              and ($7::text is null or staged_path ilike $7)
            order by {order_by}
            limit $8 offset $9
            "#
        ))
        .bind(filter.project_id)
        .bind(filter.source_id)
        .bind(filter.ingestion_status.as_deref())
        .bind(filter.inspection_status.as_deref())
        .bind(filter.extraction_status.as_deref())
        .bind(filter.media_kind.as_deref())
        .bind(
            filter
                .search
                .as_ref()
                .map(|value| format!("%{}%", value.trim())),
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let items = rows
            .iter()
            .map(media_asset_from_row)
            .collect::<Result<Vec<_>>>()?;
        Ok(MediaAssetPage {
            items,
            limit: limit as u32,
            offset: offset as u32,
        })
    }

    pub async fn delete_in_project(&self, project_id: Uuid, id: Uuid) -> Result<u64> {
        let result = sqlx::query("delete from media_assets where project_id = $1 and id = $2")
            .bind(project_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_many_in_project(&self, project_id: Uuid, ids: &[Uuid]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query("delete from media_assets where project_id = $1 and id = any($2)")
            .bind(project_id)
            .bind(ids)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
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
        extraction_status: row.try_get("extraction_status")?,
        extraction_progress: row.try_get("extraction_progress")?,
        extraction_tmp_path: row.try_get("extraction_tmp_path")?,
        extracted_at: row.try_get("extracted_at")?,
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
