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
    fn as_str(self) -> &'static str {
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

    fn parse(value: &str) -> Result<Self> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_type: SourceType,
    pub original_url: Option<String>,
    pub normalized_identity: String,
    pub import_metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SourceRepository {
    pool: PgPool,
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
        import_metadata: Option<&str>,
    ) -> Result<Source> {
        let normalized_identity = normalize_identity(source_type, original_url)?;
        let id = Uuid::new_v4();

        let row = sqlx::query(
            r#"
            insert into sources (
                id, project_id, source_type, original_url,
                normalized_identity, import_metadata, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, now(), now())
            on conflict (project_id, normalized_identity)
            do update set updated_at = excluded.updated_at
            returning id, project_id, source_type, original_url,
                      normalized_identity, import_metadata, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(project_id)
        .bind(source_type.as_str())
        .bind(original_url)
        .bind(&normalized_identity)
        .bind(import_metadata)
        .fetch_one(&self.pool)
        .await?;

        source_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Source>> {
        let row = sqlx::query(
            r#"
            select id, project_id, source_type, original_url,
                   normalized_identity, import_metadata, created_at, updated_at
            from sources
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(source_from_row).transpose()
    }

    pub async fn list(&self, project_id: Uuid) -> Result<Vec<Source>> {
        let rows = sqlx::query(
            r#"
            select id, project_id, source_type, original_url,
                   normalized_identity, import_metadata, created_at, updated_at
            from sources
            where project_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(source_from_row).collect()
    }
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

fn source_from_row(row: &PgRow) -> Result<Source> {
    Ok(Source {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        source_type: SourceType::parse(row.try_get::<String, _>("source_type")?.as_str())?,
        original_url: row.try_get("original_url")?,
        normalized_identity: row.try_get("normalized_identity")?,
        import_metadata: row.try_get("import_metadata")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
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
