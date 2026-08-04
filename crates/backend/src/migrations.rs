use anyhow::Result;
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn run(database_url: &str) -> Result<()> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect(database_url)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(())
}
pub fn applied_migrations() -> &'static [sqlx::migrate::Migration] {
    MIGRATOR.migrations.as_ref()
}
