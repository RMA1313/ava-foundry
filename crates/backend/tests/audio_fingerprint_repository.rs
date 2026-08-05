mod support;

use backend::{
    audio_asset::{AudioAssetRepository, AudioAssetUpsert},
    audio_fingerprint::{AudioFingerprintRepository, AudioFingerprintUpsert},
    migrations,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("audio_fingerprint_repo_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
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
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&schema_database_url)
        .await
        .expect("failed to connect to schema-scoped test database");
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
    .await
    .expect("failed to create fingerprint table");
    sqlx::query(
        "create index if not exists audio_fingerprints_audio_asset_idx on audio_fingerprints (audio_asset_id, created_at asc, id asc)",
    )
    .execute(&pool)
    .await
    .expect("failed to create fingerprint index");
    sqlx::query(
        "truncate table audio_fingerprints, audio_assets, media_assets, sources, projects restart identity cascade",
    )
    .execute(&pool)
    .await
    .expect("failed to clear tables");
    pool
}

async fn seed_audio_asset(pool: &PgPool) -> (Uuid, Uuid) {
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

    let audio_asset_id = Uuid::new_v4();
    let audio_repo = AudioAssetRepository::new(pool.clone());
    audio_repo
        .upsert(
            pool,
            AudioAssetUpsert {
                id: audio_asset_id,
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
                normalization_profile: Some("flac:flac:16000:1".to_string()),
                validation_status: Some("validated".to_string()),
                validation_error: None,
                validated_at: Some(chrono::Utc::now()),
            },
        )
        .await
        .expect("audio asset should insert");

    (project_id, audio_asset_id)
}

#[tokio::test]
async fn repository_round_trip_and_idempotency_work() {
    support::clear_env();
    let pool = test_pool().await;
    let (_, audio_asset_id) = seed_audio_asset(&pool).await;
    let repository = AudioFingerprintRepository::new(pool.clone());

    let first = repository
        .upsert(
            &pool,
            AudioFingerprintUpsert {
                id: Uuid::new_v4(),
                audio_asset_id,
                algorithm: "chromaprint".to_string(),
                algorithm_version: "1".to_string(),
                processing_status: "completed".to_string(),
                duration_seconds: 12.5,
                storage_path: "/data/audio/audio-fingerprints/asset.fp".to_string(),
            },
        )
        .await
        .expect("fingerprint should be created");

    let second = repository
        .upsert(
            &pool,
            AudioFingerprintUpsert {
                id: Uuid::new_v4(),
                audio_asset_id,
                algorithm: "chromaprint".to_string(),
                algorithm_version: "1".to_string(),
                processing_status: "completed".to_string(),
                duration_seconds: 12.5,
                storage_path: "/data/audio/audio-fingerprints/asset.fp".to_string(),
            },
        )
        .await
        .expect("fingerprint should be upserted");

    assert_eq!(first.id, second.id);
    assert_eq!(second.audio_asset_id, audio_asset_id);

    let fetched = repository
        .get(first.id)
        .await
        .expect("fingerprint lookup should succeed")
        .expect("fingerprint should exist");
    assert_eq!(
        fetched.storage_path,
        "/data/audio/audio-fingerprints/asset.fp"
    );
}

#[tokio::test]
async fn migration_rejects_duplicate_fingerprints_for_same_asset_algorithm_and_version() {
    support::clear_env();
    let pool = test_pool().await;
    let (_, audio_asset_id) = seed_audio_asset(&pool).await;

    sqlx::query(
        r#"
        insert into audio_fingerprints (
            id, audio_asset_id, algorithm, algorithm_version, processing_status,
            duration_seconds, storage_path, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(audio_asset_id)
    .bind("chromaprint")
    .bind("1")
    .bind("completed")
    .bind(12.5_f64)
    .bind("/data/audio/audio-fingerprints/asset.fp")
    .execute(&pool)
    .await
    .expect("fingerprint should insert");

    let duplicate = sqlx::query(
        r#"
        insert into audio_fingerprints (
            id, audio_asset_id, algorithm, algorithm_version, processing_status,
            duration_seconds, storage_path, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(audio_asset_id)
    .bind("chromaprint")
    .bind("1")
    .bind("completed")
    .bind(12.5_f64)
    .bind("/data/audio/audio-fingerprints/asset-duplicate.fp")
    .execute(&pool)
    .await;
    assert!(duplicate.is_err());

    let other_version = sqlx::query(
        r#"
        insert into audio_fingerprints (
            id, audio_asset_id, algorithm, algorithm_version, processing_status,
            duration_seconds, storage_path, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(audio_asset_id)
    .bind("chromaprint")
    .bind("2")
    .bind("completed")
    .bind(12.5_f64)
    .bind("/data/audio/audio-fingerprints/asset-v2.fp")
    .execute(&pool)
    .await;
    assert!(other_version.is_ok());
}
