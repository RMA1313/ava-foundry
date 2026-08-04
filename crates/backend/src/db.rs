use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(5).connect(database_url).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn is_ready(&self) -> bool {
        sqlx::query("select 1").execute(&self.pool).await.is_ok()
    }
}

