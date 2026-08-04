use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
        let name = validate_name(name)?;
        let description = description.map(normalize_description);
        let id = Uuid::new_v4();

        let row = sqlx::query(
            r#"
            insert into projects (id, name, description)
            values ($1, $2, $3)
            returning id, name, description, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(description.as_deref())
        .fetch_one(&self.pool)
        .await?;

        project_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Project>> {
        let row = sqlx::query(
            r#"
            select id, name, description, created_at, updated_at
            from projects
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(project_from_row).transpose()
    }

    pub async fn list(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            r#"
            select id, name, description, created_at, updated_at
            from projects
            order by created_at asc, id asc
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(project_from_row).collect()
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<Option<Project>> {
        let name = validate_name(name)?;
        let description = description.map(normalize_description);

        let row = sqlx::query(
            r#"
            update projects
            set name = $2,
                description = $3,
                updated_at = now()
            where id = $1
            returning id, name, description, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(description.as_deref())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(project_from_row).transpose()
    }
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
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn project_names_are_trimmed_and_validated() {
        assert_eq!(validate_name("  Research  ").unwrap(), "Research");
    }

    #[test]
    fn project_names_cannot_be_empty() {
        assert!(validate_name("   ").is_err());
    }
}
