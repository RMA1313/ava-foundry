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
    sqlx::query(
        r#"
        create table if not exists audio_fingerprints (
            id uuid primary key,
            audio_asset_id uuid not null references audio_assets(id) on delete restrict,
            algorithm text not null,
            algorithm_version text not null,
            processing_status text not null,
            duration_seconds double precision not null,
            storage_path text not null,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now(),
            unique (audio_asset_id, algorithm, algorithm_version)
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        create index if not exists audio_fingerprints_audio_asset_idx
            on audio_fingerprints (audio_asset_id, created_at asc, id asc)
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        create table if not exists duplicate_groups (
            id uuid primary key,
            project_id uuid not null references projects(id) on delete restrict,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        create table if not exists duplicate_group_memberships (
            duplicate_group_id uuid not null references duplicate_groups(id) on delete cascade,
            audio_asset_id uuid not null references audio_assets(id) on delete restrict,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now(),
            unique (audio_asset_id)
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        create table if not exists audio_duplicate_decisions (
            id uuid primary key,
            audio_asset_low_id uuid not null references audio_assets(id) on delete restrict,
            audio_asset_high_id uuid not null references audio_assets(id) on delete restrict,
            candidate_fingerprint_low_id uuid not null references audio_fingerprints(id) on delete restrict,
            candidate_fingerprint_high_id uuid not null references audio_fingerprints(id) on delete restrict,
            algorithm text not null,
            algorithm_version text not null,
            duration_delta_seconds double precision not null,
            similarity_score double precision not null,
            decision text not null,
            confidence text not null,
            duplicate_group_id uuid references duplicate_groups(id) on delete set null,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now(),
            unique (audio_asset_low_id, audio_asset_high_id, algorithm, algorithm_version)
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        create index if not exists duplicate_group_memberships_group_idx
            on duplicate_group_memberships (duplicate_group_id, audio_asset_id asc)
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        create index if not exists audio_duplicate_decisions_lookup_idx
            on audio_duplicate_decisions (algorithm, algorithm_version, decision, similarity_score desc, created_at asc, id asc)
        "#,
    )
    .execute(&pool)
    .await?;
    Ok(())
}
pub fn applied_migrations() -> &'static [sqlx::migrate::Migration] {
    MIGRATOR.migrations.as_ref()
}
