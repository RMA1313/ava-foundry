use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::job::JobRepository;
use crate::project::ProjectRepository;
use crate::source::SourceRepository;

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn projects(&self) -> ProjectRepository {
        ProjectRepository::new(self.pool.clone())
    }

    pub fn jobs(&self) -> JobRepository {
        JobRepository::new(self.pool.clone())
    }

    pub fn sources(&self) -> SourceRepository {
        SourceRepository::new(self.pool.clone())
    }

    pub async fn is_ready(&self) -> bool {
        sqlx::query("select 1").execute(&self.pool).await.is_ok()
    }
}
