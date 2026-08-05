use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    LocalFile,
    LocalFolder,
    PublicUrl,
    CsvImport,
    ExistingDataset,
    HuggingFaceDataset,
    Archive,
}

impl SourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalFile => "local_file",
            Self::LocalFolder => "local_folder",
            Self::PublicUrl => "public_url",
            Self::CsvImport => "csv_import",
            Self::ExistingDataset => "existing_dataset",
            Self::HuggingFaceDataset => "hugging_face_dataset",
            Self::Archive => "archive",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "local_file" => Ok(Self::LocalFile),
            "local_folder" => Ok(Self::LocalFolder),
            "public_url" => Ok(Self::PublicUrl),
            "csv_import" => Ok(Self::CsvImport),
            "existing_dataset" => Ok(Self::ExistingDataset),
            "hugging_face_dataset" => Ok(Self::HuggingFaceDataset),
            "archive" => Ok(Self::Archive),
            other => Err(anyhow!("invalid source type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Pending,
    Staged,
    Completed,
    Failed,
}

impl SourceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Staged => "staged",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "staged" => Ok(Self::Staged),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(anyhow!("invalid source status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_type: SourceType,
    pub original_url: Option<String>,
    pub normalized_identity: String,
    pub source_config: Option<String>,
    pub failure_reason: Option<String>,
    pub import_metadata: Option<String>,
    pub file_count: i64,
    pub byte_count: i64,
    pub ingestion_status: SourceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SourceRepository {
    pool: PgPool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceListFilter {
    pub source_type: Option<SourceType>,
    pub ingestion_status: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSortField {
    CreatedAt,
    UpdatedAt,
    SourceType,
}

impl SourceSortField {
    fn as_sql(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::SourceType => "source_type",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "created_at" => Ok(Self::CreatedAt),
            "updated_at" => Ok(Self::UpdatedAt),
            "source_type" => Ok(Self::SourceType),
            other => Err(anyhow!("invalid source sort field: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceListSort {
    pub field: SourceSortField,
    pub descending: bool,
}

impl Default for SourceListSort {
    fn default() -> Self {
        Self {
            field: SourceSortField::CreatedAt,
            descending: false,
        }
    }
}

impl SourceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        project_id: Uuid,
        source_type: SourceType,
        original_url: Option<&str>,
        source_config: Option<&str>,
        import_metadata: Option<&str>,
    ) -> Result<Source> {
        let normalized_identity = normalize_identity(source_type, original_url)?;
        let id = Uuid::new_v4();

        let row = sqlx::query(
            r#"
            insert into sources (
                id, project_id, source_type, original_url,
                normalized_identity, source_config, failure_reason, import_metadata,
                file_count, byte_count, ingestion_status, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, null, $7, 0, 0, 'pending', now(), now())
            on conflict (project_id, normalized_identity)
            do update set updated_at = excluded.updated_at
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(project_id)
        .bind(source_type.as_str())
        .bind(original_url)
        .bind(&normalized_identity)
        .bind(source_config)
        .bind(import_metadata)
        .fetch_one(&self.pool)
        .await?;

        source_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Source>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_type, original_url,
                   normalized_identity, source_config, failure_reason, import_metadata,
                   file_count, byte_count, ingestion_status, created_at, updated_at
            from sources
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn get_in_project(&self, project_id: Uuid, id: Uuid) -> Result<Option<Source>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_type, original_url,
                   normalized_identity, source_config, failure_reason, import_metadata,
                   file_count, byte_count, ingestion_status, created_at, updated_at
            from sources
            where project_id = $1 and id = $2
            "#,
        )
        .bind(project_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn list(&self, project_id: Uuid) -> Result<Vec<Source>> {
        self.list_paginated(project_id, None, None, 100, 0)
            .await
            .map(|page| page.items)
    }

    pub async fn list_paginated(
        &self,
        project_id: Uuid,
        filter: Option<SourceListFilter>,
        sort: Option<SourceListSort>,
        limit: i64,
        offset: i64,
    ) -> Result<SourcePage> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let (source_type, ingestion_status, search) = match filter {
            Some(filter) => (
                filter.source_type.map(|value| value.as_str()),
                filter.ingestion_status,
                filter.search.map(|value| format!("%{}%", value.trim())),
            ),
            None => (None, None, None),
        };
        let sort = sort.unwrap_or_default();
        let order_by = format!(
            "{} {}, id {}",
            sort.field.as_sql(),
            if sort.descending { "desc" } else { "asc" },
            if sort.descending { "desc" } else { "asc" }
        );
        let rows = sqlx::query(
            &format!(
                r#"
            select id, project_id, source_type, original_url,
                   normalized_identity, source_config, failure_reason, import_metadata,
                   file_count, byte_count, ingestion_status, created_at, updated_at
            from sources
            where project_id = $1
              and ($2::text is null or source_type = $2)
              and ($3::text is null or ingestion_status = $3)
              and ($4::text is null or normalized_identity ilike $4 or original_url ilike $4 or source_config ilike $4 or import_metadata ilike $4)
            order by {order_by}
            limit $5 offset $6
            "#
            ),
        )
        .bind(project_id)
        .bind(source_type)
        .bind(ingestion_status)
        .bind(search.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let items = rows
            .iter()
            .map(source_from_row)
            .collect::<Result<Vec<_>>>()?;
        Ok(SourcePage {
            items,
            limit: limit as u32,
            offset: offset as u32,
        })
    }

    pub async fn get_by_identity(
        &self,
        project_id: Uuid,
        normalized_identity: &str,
    ) -> Result<Option<Source>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_type, original_url,
                   normalized_identity, source_config, failure_reason, import_metadata,
                   file_count, byte_count, ingestion_status, created_at, updated_at
            from sources
            where project_id = $1 and normalized_identity = $2
            "#,
        )
        .bind(project_id)
        .bind(normalized_identity)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn update_import_metadata(
        &self,
        id: Uuid,
        import_metadata: Option<&str>,
    ) -> Result<Option<Source>> {
        let row = sqlx::query(
            r#"
            update sources
            set import_metadata = $2,
                updated_at = now()
            where id = $1
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(import_metadata)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn update_source_config(
        &self,
        id: Uuid,
        source_config: Option<&str>,
    ) -> Result<Option<Source>> {
        let row = sqlx::query(
            r#"
            update sources
            set source_config = $2,
                updated_at = now()
            where id = $1
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(source_config)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn update_failure_state(
        &self,
        id: Uuid,
        failure_reason: Option<&str>,
    ) -> Result<Option<Source>> {
        let current = self.get(id).await?;
        let current = match current {
            Some(source) => source,
            None => return Ok(None),
        };
        if failure_reason.is_some()
            && !can_transition(current.ingestion_status, SourceStatus::Failed)
        {
            return Err(anyhow!("invalid source status transition"));
        }
        let row = sqlx::query(
            r#"
            update sources
            set failure_reason = $2,
                ingestion_status = case
                    when $2 is null then ingestion_status
                    else 'failed'
                end,
                updated_at = now()
            where id = $1
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(failure_reason)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn update_counters(
        &self,
        id: Uuid,
        file_count: i64,
        byte_count: i64,
    ) -> Result<Option<Source>> {
        if file_count < 0 || byte_count < 0 {
            return Err(anyhow!("source counters cannot be negative"));
        }
        let row = sqlx::query(
            r#"
            update sources
            set file_count = $2,
                byte_count = $3,
                updated_at = now()
            where id = $1
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(file_count)
        .bind(byte_count)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn update_ingestion_status(
        &self,
        id: Uuid,
        ingestion_status: &str,
    ) -> Result<Option<Source>> {
        let ingestion_status = SourceStatus::parse(ingestion_status)?;
        let current = self.get(id).await?;
        let current = match current {
            Some(source) => source,
            None => return Ok(None),
        };
        if !can_transition(current.ingestion_status, ingestion_status) {
            return Err(anyhow!("invalid source status transition"));
        }
        let row = sqlx::query(
            r#"
            update sources
            set ingestion_status = $2,
                updated_at = now()
            where id = $1
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(ingestion_status.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn cancel(&self, project_id: Uuid, id: Uuid) -> Result<Option<Source>> {
        let current = self.get_in_project(project_id, id).await?;
        let current = match current {
            Some(source) => source,
            None => return Ok(None),
        };
        if !matches!(
            current.ingestion_status,
            SourceStatus::Pending | SourceStatus::Staged
        ) {
            return Err(anyhow!("invalid source status transition"));
        }
        self.update_failure_state(id, Some("cancelled")).await
    }

    pub async fn retry(&self, project_id: Uuid, id: Uuid) -> Result<Option<Source>> {
        let current = self.get_in_project(project_id, id).await?;
        let current = match current {
            Some(source) => source,
            None => return Ok(None),
        };
        if current.ingestion_status != SourceStatus::Failed {
            return Err(anyhow!("invalid source status transition"));
        }
        let row = sqlx::query(
            r#"
            update sources
            set ingestion_status = 'pending',
                failure_reason = null,
                updated_at = now()
            where id = $1
            returning id, project_id, source_type, original_url,
                      normalized_identity, source_config, failure_reason, import_metadata,
                      file_count, byte_count, ingestion_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn delete(&self, project_id: Uuid, id: Uuid) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from sources where project_id = $1 and id = $2)",
        )
        .bind(project_id)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if !deleted {
            tx.commit().await?;
            return Ok(false);
        }
        sqlx::query(
            "delete from stage_runs where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2)",
        )
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from audio_duplicate_decisions where audio_asset_low_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2)) or audio_asset_high_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2)) or candidate_fingerprint_low_id in (select id from audio_fingerprints where audio_asset_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2))) or candidate_fingerprint_high_id in (select id from audio_fingerprints where audio_asset_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2)))",
        )
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from duplicate_group_memberships where audio_asset_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2))",
        )
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from duplicate_groups where project_id = $1 and canonical_audio_asset_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2))",
        )
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from audio_fingerprints where audio_asset_id in (select id from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2))",
        )
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from audio_assets where project_id = $1 and media_asset_id in (select id from media_assets where project_id = $1 and source_id = $2)",
        )
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("delete from media_assets where project_id = $1 and source_id = $2")
            .bind(project_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from jobs where project_id = $1 and source_id = $2")
            .bind(project_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from sources where project_id = $1 and id = $2")
            .bind(project_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(deleted)
    }
}

fn can_transition(from: SourceStatus, to: SourceStatus) -> bool {
    matches!(
        (from, to),
        (SourceStatus::Pending, SourceStatus::Pending)
            | (SourceStatus::Pending, SourceStatus::Staged)
            | (SourceStatus::Pending, SourceStatus::Completed)
            | (SourceStatus::Pending, SourceStatus::Failed)
            | (SourceStatus::Staged, SourceStatus::Staged)
            | (SourceStatus::Staged, SourceStatus::Completed)
            | (SourceStatus::Staged, SourceStatus::Failed)
            | (SourceStatus::Completed, SourceStatus::Completed)
            | (SourceStatus::Failed, SourceStatus::Failed)
    )
}

fn normalize_identity(source_type: SourceType, original_url: Option<&str>) -> Result<String> {
    match source_type {
        SourceType::PublicUrl => {
            let url = original_url.ok_or_else(|| anyhow!("public url source requires a url"))?;
            Ok(format!("url:{}", normalize_url(url)))
        }
        _ => Ok(format!(
            "{}:{}",
            source_type.as_str(),
            original_url.unwrap_or_default().trim()
        )),
    }
}

fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("https://youtu.be/") {
        return format!("youtube:{}", rest.trim_matches('/'));
    }
    if let Some(rest) = trimmed.strip_prefix("https://www.youtube.com/watch?v=") {
        return format!("youtube:{}", rest.split('&').next().unwrap_or(rest));
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    without_query.trim_end_matches('/').to_lowercase()
}

pub(crate) fn source_from_row(row: &PgRow) -> Result<Source> {
    Ok(Source {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        source_type: SourceType::parse(row.try_get::<String, _>("source_type")?.as_str())?,
        original_url: row.try_get("original_url")?,
        normalized_identity: row.try_get("normalized_identity")?,
        source_config: row.try_get("source_config")?,
        failure_reason: row.try_get("failure_reason")?,
        import_metadata: row.try_get("import_metadata")?,
        file_count: row.try_get("file_count")?,
        byte_count: row.try_get("byte_count")?,
        ingestion_status: SourceStatus::parse(
            row.try_get::<String, _>("ingestion_status")?.as_str(),
        )?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePage {
    pub items: Vec<Source>,
    pub limit: u32,
    pub offset: u32,
}

#[cfg(test)]
mod tests {
    use super::{normalize_identity, SourceType};

    #[test]
    fn public_urls_normalize_deterministically() {
        let a = normalize_identity(
            SourceType::PublicUrl,
            Some("https://www.youtube.com/watch?v=abcd1234&t=12"),
        )
        .unwrap();
        let b =
            normalize_identity(SourceType::PublicUrl, Some("https://youtu.be/abcd1234")).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn generic_urls_are_stable() {
        let a =
            normalize_identity(SourceType::PublicUrl, Some("https://example.com/a/?x=1")).unwrap();
        let b = normalize_identity(SourceType::PublicUrl, Some("https://example.com/a")).unwrap();
        assert_eq!(a, b);
    }
}
