mod support;

use backend::{
    audio_asset::{AudioAssetRepository, AudioAssetUpsert},
    migrations,
};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("audio_asset_repo_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPool::connect(&database_url)
        .await
        .expect("failed to connect to test database");
    sqlx::query(&format!("create schema {}", schema_name))
        .execute(&admin_pool)
        .await
        .expect("failed to create test schema");
    let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
    migrations::run(&schema_database_url)
        .await
        .expect("failed to run migrations");
    let pool = PgPool::connect(&schema_database_url)
        .await
        .expect("failed to connect to schema-scoped test database");
    sqlx::query(
        "truncate table audio_assets, media_assets, sources, projects restart identity cascade",
    )
    .execute(&pool)
    .await
    .expect("failed to clear media tables");
    pool
}

async fn seed_project_source_and_media(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let project_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into projects (id, name, description, created_at, updated_at)
        values ($1, $2, $3, now(), now())
        "#,
    )
    .bind(project_id)
    .bind("Research")
    .bind(Option::<String>::None)
    .execute(pool)
    .await
    .expect("project should insert");

    let source_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
        values ($1, $2, $3, $4, $5, $6, $7, now(), now())
        "#,
    )
    .bind(source_id)
    .bind(project_id)
    .bind("public_url")
    .bind(Some("https://youtu.be/abcd1234"))
    .bind("url:youtube:abcd1234")
    .bind(Option::<String>::None)
    .bind("staged")
    .execute(pool)
    .await
    .expect("source should insert");

    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
        "#,
    )
    .bind(media_asset_id)
    .bind(project_id)
    .bind(source_id)
    .bind("video")
    .bind("/tmp/staged/source.bin")
    .bind(Some("source.mp4"))
    .bind(12_i64)
    .bind("ready")
    .execute(pool)
    .await
    .expect("media asset should insert");

    (project_id, source_id, media_asset_id)
}

#[tokio::test]
async fn repository_round_trip_and_idempotency_work() {
    support::clear_env();
    let pool = test_pool().await;
    let (project_id, _, media_asset_id) = seed_project_source_and_media(&pool).await;
    let repository = AudioAssetRepository::new(pool.clone());

    let first = repository
        .upsert(
            &pool,
            AudioAssetUpsert {
                media_asset_id,
                project_id,
                processing_status: "validated".to_string(),
                duration_seconds: 12.5,
                audio_format: "flac".to_string(),
                codec: "flac".to_string(),
                sample_rate_hz: 16000,
                channels: 1,
                byte_size: 1024,
                storage_path: "/data/audio/audio-assets/asset.flac".to_string(),
            },
        )
        .await
        .expect("audio asset should be created");

    let second = repository
        .upsert(
            &pool,
            AudioAssetUpsert {
                media_asset_id,
                project_id,
                processing_status: "validated".to_string(),
                duration_seconds: 12.5,
                audio_format: "flac".to_string(),
                codec: "flac".to_string(),
                sample_rate_hz: 16000,
                channels: 1,
                byte_size: 1024,
                storage_path: "/data/audio/audio-assets/asset.flac".to_string(),
            },
        )
        .await
        .expect("audio asset should be upserted");

    assert_eq!(first.id, second.id);
    assert_eq!(second.media_asset_id, media_asset_id);

    let fetched = repository
        .get(first.id)
        .await
        .expect("audio asset lookup should succeed")
        .expect("audio asset should exist");
    assert_eq!(fetched.storage_path, "/data/audio/audio-assets/asset.flac");

    let listed = repository
        .list(project_id)
        .await
        .expect("audio asset list should succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, first.id);
}

#[tokio::test]
async fn migration_rejects_invalid_relationships_and_duplicates() {
    support::clear_env();
    let pool = test_pool().await;
    let (project_id, _, media_asset_id) = seed_project_source_and_media(&pool).await;

    sqlx::query(
        r#"
        insert into audio_assets (
            id, media_asset_id, project_id, processing_status, duration_seconds,
            audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
            created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(media_asset_id)
    .bind(project_id)
    .bind("validated")
    .bind(12.5_f64)
    .bind("flac")
    .bind("flac")
    .bind(16000_i32)
    .bind(1_i32)
    .bind(1024_i64)
    .bind("/data/audio/audio-assets/asset.flac")
    .execute(&pool)
    .await
    .expect("audio asset should insert through migration schema");

    let duplicate = sqlx::query(
        r#"
        insert into audio_assets (
            id, media_asset_id, project_id, processing_status, duration_seconds,
            audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
            created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(media_asset_id)
    .bind(project_id)
    .bind("validated")
    .bind(12.5_f64)
    .bind("flac")
    .bind("flac")
    .bind(16000_i32)
    .bind(1_i32)
    .bind(1024_i64)
    .bind("/data/audio/audio-assets/asset.flac")
    .execute(&pool)
    .await;
    assert!(duplicate.is_err());

    let orphan = sqlx::query(
        r#"
        insert into audio_assets (
            id, media_asset_id, project_id, processing_status, duration_seconds,
            audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
            created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind("validated")
    .bind(12.5_f64)
    .bind("flac")
    .bind("flac")
    .bind(16000_i32)
    .bind(1_i32)
    .bind(1024_i64)
    .bind("/data/audio/audio-assets/orphan.flac")
    .execute(&pool)
    .await;
    assert!(orphan.is_err());
}
