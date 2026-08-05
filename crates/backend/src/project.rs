use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, PgPool, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    Active,
    Archived,
}

impl ProjectStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            other => Err(anyhow!("invalid project status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSortField {
    CreatedAt,
    UpdatedAt,
    Name,
}

impl ProjectSortField {
    fn as_sql(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::Name => "name",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "created_at" => Ok(Self::CreatedAt),
            "updated_at" => Ok(Self::UpdatedAt),
            "name" => Ok(Self::Name),
            other => Err(anyhow!("invalid project sort field: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "asc" => Ok(Self::Asc),
            "desc" => Ok(Self::Desc),
            other => Err(anyhow!("invalid sort direction: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectListFilter {
    pub status: Option<ProjectStatus>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectListSort {
    pub field: ProjectSortField,
    pub direction: SortDirection,
}

impl Default for ProjectListSort {
    fn default() -> Self {
        Self {
            field: ProjectSortField::CreatedAt,
            direction: SortDirection::Asc,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPage {
    pub items: Vec<Project>,
    pub limit: u32,
    pub offset: u32,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct ProjectRepository {
    pool: PgPool,
}

impl ProjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, name: &str, description: Option<&str>) -> Result<Project> {
        let mut tx = self.pool.begin().await?;
        let project = self.create_in_tx(&mut tx, name, description).await?;
        tx.commit().await?;
        Ok(project)
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, sqlx::Postgres>,
        name: &str,
        description: Option<&str>,
    ) -> Result<Project> {
        let name = validate_name(name)?;
        let description = description.map(normalize_description);
        let id = Uuid::new_v4();
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            insert into projects (id, name, description, status, created_at, updated_at)
            values ($1, $2, $3, 'active', $4, $4)
            returning id, name, description, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(description.as_deref())
        .bind(now)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_project_unique_violation)?;
        project_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Project>> {
        let row = sqlx::query(
            r#"
            select id, name, description, status, created_at, updated_at
            from projects
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_project_unique_violation)?;
        row.as_ref().map(project_from_row).transpose()
    }

    pub async fn list(&self) -> Result<Vec<Project>> {
        self.list_paginated(None, None, 100, 0)
            .await
            .map(|page| page.items)
    }

    pub async fn list_paginated(
        &self,
        filter: Option<ProjectListFilter>,
        sort: Option<ProjectListSort>,
        limit: i64,
        offset: i64,
    ) -> Result<ProjectPage> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let filter = filter.unwrap_or(ProjectListFilter {
            status: None,
            search: None,
        });
        let sort = sort.unwrap_or_default();
        let search = filter
            .search
            .as_deref()
            .map(|value| format!("%{}%", value.trim()));
        let status = filter.status.map(|s| s.as_str());
        let order_by = format!(
            "{} {}, id {}",
            sort.field.as_sql(),
            sort.direction.as_sql(),
            sort.direction.as_sql()
        );

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)
            from projects
            where ($1::text is null or status = $1)
              and ($2::text is null or name ilike $2)
            "#,
        )
        .bind(status)
        .bind(search.as_deref())
        .fetch_one(&self.pool)
        .await?;

        let query = format!(
            "select id, name, description, status, created_at, updated_at from projects where ($1::text is null or status = $1) and ($2::text is null or name ilike $2) order by {order_by} limit $3 offset $4"
        );
        let rows = sqlx::query(&query)
            .bind(status)
            .bind(search.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let items = rows
            .iter()
            .map(project_from_row)
            .collect::<Result<Vec<_>>>()?;
        Ok(ProjectPage {
            items,
            limit: limit as u32,
            offset: offset as u32,
            total,
        })
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        status: Option<ProjectStatus>,
    ) -> Result<Option<Project>> {
        let name = validate_name(name)?;
        let description = description.map(normalize_description);
        let row = sqlx::query(
            r#"
            update projects
            set name = $2,
                description = $3,
                status = coalesce($4, status),
                updated_at = now()
            where id = $1
            returning id, name, description, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(description.as_deref())
        .bind(status.map(|v| v.as_str()))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_project_unique_violation)?;
        row.as_ref().map(project_from_row).transpose()
    }

    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let deleted = self.delete_in_tx(&mut tx, id).await?;
        tx.commit().await?;
        Ok(deleted)
    }

    pub async fn delete_in_tx(
        &self,
        tx: &mut Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<bool> {
        let deleted = sqlx::query(
            r#"
            with deleted_stage_runs as (
                delete from stage_runs where project_id = $1
            ),
            deleted_duplicate_decisions as (
                delete from audio_duplicate_decisions
                where audio_asset_low_id in (select id from audio_assets where project_id = $1)
                   or audio_asset_high_id in (select id from audio_assets where project_id = $1)
            ),
            deleted_duplicate_memberships as (
                delete from duplicate_group_memberships
                where duplicate_group_id in (select id from duplicate_groups where project_id = $1)
            ),
            deleted_duplicate_groups as (
                delete from duplicate_groups where project_id = $1
            ),
            deleted_audio_fingerprints as (
                delete from audio_fingerprints where audio_asset_id in (select id from audio_assets where project_id = $1)
            ),
            deleted_audio_assets as (
                delete from audio_assets where project_id = $1
            ),
            deleted_media_assets as (
                delete from media_assets where project_id = $1
            ),
            deleted_jobs as (
                delete from jobs where project_id = $1
            ),
            deleted_sources as (
                delete from sources where project_id = $1
            )
            delete from projects where id = $1
            returning id
            "#,
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_project_unique_violation)?
        .is_some();
        Ok(deleted)
    }
}

fn map_project_unique_violation(err: sqlx::Error) -> anyhow::Error {
    if let sqlx::Error::Database(db_error) = &err {
        if db_error.code().as_deref() == Some("23505")
            && db_error
                .constraint()
                .map(|name| name == "projects_name_unique_idx")
                .unwrap_or(false)
        {
            return anyhow!("project name already exists");
        }
    }
    err.into()
}

fn validate_name(name: &str) -> Result<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(anyhow!("project name cannot be empty"));
    }
    if normalized.len() > 120 {
        return Err(anyhow!("project name must be 120 characters or fewer"));
    }
    Ok(normalized.to_string())
}

fn normalize_description(description: &str) -> String {
    description.trim().to_string()
}

fn project_from_row(row: &PgRow) -> Result<Project> {
    Ok(Project {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        status: ProjectStatus::parse(row.try_get::<String, _>("status")?.as_str())?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::{validate_name, ProjectStatus};

    #[test]
    fn project_names_are_trimmed_and_validated() {
        assert_eq!(validate_name("  Research  ").unwrap(), "Research");
    }

    #[test]
    fn project_names_cannot_be_empty() {
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn project_names_are_limited_to_120_characters() {
        let long_name = "a".repeat(121);
        assert!(validate_name(&long_name).is_err());
    }

    #[test]
    fn project_statuses_parse() {
        assert_eq!(
            ProjectStatus::parse("active").unwrap(),
            ProjectStatus::Active
        );
        assert!(ProjectStatus::parse("bad").is_err());
    }
}
