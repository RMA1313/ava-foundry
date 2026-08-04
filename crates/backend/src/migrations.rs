use anyhow::Result;
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn run(database_url: &str) -> Result<()> {
    // Keep this module changing when migrations change so SQLx re-embeds the set.
    // Phase 3 adds fingerprint persistence in a later migration.
    let _ = database_url;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect(database_url)
        .await?;
    MIGRATOR.run(&pool).await?;
    sqlx::query(
        r#"
        alter table if exists media_assets
            add column if not exists inspection_status text,
            add column if not exists container_format text,
            add column if not exists duration_seconds double precision,
            add column if not exists codecs text,
            add column if not exists sample_rate_hz integer,
            add column if not exists channels integer,
            add column if not exists inspected_at timestamptz
        "#,
    )
    .execute(&pool)
    .await?;
    Ok(())
}
pub fn applied_migrations() -> &'static [sqlx::migrate::Migration] {
    MIGRATOR.migrations.as_ref()
}
