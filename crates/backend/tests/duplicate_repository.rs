mod support;

use backend::{
    audio_asset::{AudioAssetRepository, AudioAssetUpsert},
    duplicate::DuplicateRepository,
    migrations,
};
use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("duplicate_repo_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("connect");
    sqlx::query(&format!("create schema {}", schema_name))
        .execute(&admin_pool)
        .await
        .expect("create schema");
    drop(admin_pool);
    let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
    migrations::run(&schema_database_url)
        .await
        .expect("migrations");
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&schema_database_url)
        .await
        .expect("connect schema pool")
}

async fn seed_project(pool: &PgPool, name: &str) -> Uuid {
    let project_id = Uuid::new_v4();
    sqlx::query(
        "insert into projects (id, name, created_at, updated_at) values ($1, $2, now(), now())",
    )
    .bind(project_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("project");
    project_id
}

async fn seed_asset(pool: &PgPool, project_id: Uuid, suffix: &str, duration_seconds: f64) -> Uuid {
    let source_id = Uuid::new_v4();
    sqlx::query(
        "insert into sources (id, project_id, source_type, original_url, normalized_identity, ingestion_status, created_at, updated_at) values ($1, $2, $3, $4, $5, $6, now(), now())",
    )
    .bind(source_id)
    .bind(project_id)
    .bind("public_url")
    .bind(Some(format!("https://example.com/{suffix}")))
    .bind(format!("url:example:{suffix}"))
    .bind("staged")
    .execute(pool)
    .await
    .expect("source");

    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        "insert into media_assets (id, project_id, source_id, media_kind, staged_path, byte_size, ingestion_status, created_at, updated_at) values ($1, $2, $3, $4, $5, $6, $7, now(), now())",
    )
    .bind(media_asset_id)
    .bind(project_id)
    .bind(source_id)
    .bind("video")
    .bind(format!("/tmp/staged/{suffix}.bin"))
    .bind(1_i64)
    .bind("ready")
    .execute(pool)
    .await
    .expect("media");

    let audio_asset_id = Uuid::new_v4();
    AudioAssetRepository::new(pool.clone())
        .upsert(
            pool,
            AudioAssetUpsert {
                id: audio_asset_id,
                media_asset_id,
                project_id,
                processing_status: "validated".to_string(),
                normalization_profile: None,
                validation_status: Some("validated".to_string()),
                validation_error: None,
                validated_at: Some(Utc::now()),
                duration_seconds,
                audio_format: "flac".to_string(),
                codec: "flac".to_string(),
                sample_rate_hz: 16000,
                channels: 1,
                byte_size: 10,
                storage_path: format!("/audio/{audio_asset_id}.flac"),
            },
        )
        .await
        .expect("audio");
    audio_asset_id
}

#[tokio::test]
async fn canonical_selection_is_deterministic_and_idempotent() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let first_asset = seed_asset(&pool, project_id, "a", 3.0).await;
    let second_asset = seed_asset(&pool, project_id, "b", 3.0).await;
    let repo = DuplicateRepository::new(pool.clone());
    let group_id = repo
        .ensure_group_membership(project_id, [second_asset, first_asset])
        .await
        .expect("group");
    let selected = repo
        .ensure_canonical_audio_asset(group_id)
        .await
        .expect("canonical");
    let expected = Some(if first_asset < second_asset {
        first_asset
    } else {
        second_asset
    });
    assert_eq!(selected, expected);
    let repeated = repo
        .ensure_canonical_audio_asset(group_id)
        .await
        .expect("repeat canonical");
    assert_eq!(selected, repeated);
}

#[tokio::test]
async fn recovery_fills_missing_canonical_only_once() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let first_asset = seed_asset(&pool, project_id, "a", 3.0).await;
    let second_asset = seed_asset(&pool, project_id, "b", 3.0).await;
    let repo = DuplicateRepository::new(pool.clone());
    let group_id = repo
        .ensure_group_membership(project_id, [first_asset, second_asset])
        .await
        .expect("group");
    sqlx::query("update duplicate_groups set canonical_audio_asset_id = null where id = $1")
        .bind(group_id)
        .execute(&pool)
        .await
        .expect("clear canonical");
    let recovered = repo
        .recover_canonical_audio_assets()
        .await
        .expect("recover");
    let expected = if first_asset < second_asset {
        first_asset
    } else {
        second_asset
    };
    assert_eq!(recovered, vec![(group_id, expected)]);
    let page = repo.list_groups(project_id, 10, 0).await.expect("list");
    assert_eq!(page.items[0].group.canonical_audio_asset_id, Some(expected));
}

#[tokio::test]
async fn duplicate_groups_are_project_scoped_and_paginated_deterministically() {
    support::clear_env();
    let pool = test_pool().await;
    let project_a = seed_project(&pool, "Research A").await;
    let project_b = seed_project(&pool, "Research B").await;
    let asset_a1 = seed_asset(&pool, project_a, "a1", 3.0).await;
    let asset_a2 = seed_asset(&pool, project_a, "a2", 3.0).await;
    let asset_b1 = seed_asset(&pool, project_b, "b1", 3.0).await;
    let asset_b2 = seed_asset(&pool, project_b, "b2", 3.0).await;
    let repo = DuplicateRepository::new(pool.clone());
    let group_a = repo
        .ensure_group_membership(project_a, [asset_a1, asset_a2])
        .await
        .expect("group a");
    let group_b = repo
        .ensure_group_membership(project_b, [asset_b1, asset_b2])
        .await
        .expect("group b");
    repo.ensure_canonical_audio_asset(group_a)
        .await
        .expect("canon a");
    repo.ensure_canonical_audio_asset(group_b)
        .await
        .expect("canon b");
    let page = repo.list_groups(project_a, 1, 0).await.expect("page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].group.project_id, project_a);
}
