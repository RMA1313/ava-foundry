mod support;

use backend::{
    migrations,
    source::{SourceRepository, SourceType},
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
    let schema_name = format!("source_repo_test_{}", Uuid::new_v4().simple());
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
    sqlx::query("truncate table sources restart identity cascade")
        .execute(&pool)
        .await
        .expect("failed to clear sources table");
    pool
}

async fn seed_project(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into projects (id, name, description, created_at, updated_at)
        values ($1, $2, $3, now(), now())
        "#,
    )
    .bind(id)
    .bind("Research")
    .bind(Option::<String>::None)
    .execute(pool)
    .await
    .expect("project should insert");
    id
}

#[tokio::test]
async fn repository_round_trip_works() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = SourceRepository::new(pool.clone());

    let created = repository
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://www.youtube.com/watch?v=abcd1234&t=12"),
            None,
            Some(r#"{"origin":"manual"}"#),
        )
        .await
        .expect("source should be created");

    assert_eq!(created.project_id, project_id);
    assert_eq!(created.source_type, SourceType::PublicUrl);
    assert_eq!(created.normalized_identity, "url:youtube:abcd1234");

    let fetched = repository
        .get(created.id)
        .await
        .expect("source lookup should succeed")
        .expect("source should exist");
    assert_eq!(fetched.id, created.id);

    let listed = repository
        .list(project_id)
        .await
        .expect("source list should succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[tokio::test]
async fn repository_returns_existing_source_deterministically() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = SourceRepository::new(pool);

    let first = repository
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/abcd1234"),
            None,
            None,
        )
        .await
        .expect("source should be created");
    let second = repository
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://www.youtube.com/watch?v=abcd1234"),
            None,
            None,
        )
        .await
        .expect("source should resolve deterministically");

    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn repository_rejects_invalid_project_relationships() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = SourceRepository::new(pool);

    let err = repository
        .create(
            Uuid::new_v4(),
            SourceType::LocalFile,
            Some("/tmp/audio.wav"),
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("violates foreign key constraint"));
}

#[tokio::test]
async fn repository_persists_source_configuration_counters_and_failure_state() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = SourceRepository::new(pool);

    let created = repository
        .create(
            project_id,
            SourceType::LocalFolder,
            Some("/tmp/library"),
            None,
            Some(r#"{"recursive":true}"#),
        )
        .await
        .expect("source should be created");

    let configured = repository
        .update_source_config(created.id, Some(r#"{"recursive":false}"#))
        .await
        .expect("config update should succeed")
        .expect("source should exist");
    assert_eq!(
        configured.source_config.as_deref(),
        Some(r#"{"recursive":false}"#)
    );

    let counted = repository
        .update_counters(created.id, 12, 4096)
        .await
        .expect("counter update should succeed")
        .expect("source should exist");
    assert_eq!(counted.file_count, 12);
    assert_eq!(counted.byte_count, 4096);

    let failed = repository
        .update_failure_state(created.id, Some("scan failed"))
        .await
        .expect("failure update should succeed")
        .expect("source should exist");
    assert_eq!(failed.ingestion_status.as_str(), "failed");
    assert_eq!(failed.failure_reason.as_deref(), Some("scan failed"));
}

#[tokio::test]
async fn repository_rejects_invalid_source_status_values() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = SourceRepository::new(pool);

    let source = repository
        .create(
            project_id,
            SourceType::LocalFile,
            Some("/tmp/audio.wav"),
            None,
            None,
        )
        .await
        .expect("source should be created");

    let err = repository
        .update_ingestion_status(source.id, "queued")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid source status"));
}

#[tokio::test]
async fn repository_rejects_invalid_source_status_transitions() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = SourceRepository::new(pool);

    let source = repository
        .create(
            project_id,
            SourceType::LocalFile,
            Some("/tmp/audio.wav"),
            None,
            None,
        )
        .await
        .expect("source should be created");

    repository
        .update_ingestion_status(source.id, "completed")
        .await
        .expect("completion should succeed");

    let err = repository
        .update_ingestion_status(source.id, "staged")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid source status transition"));
}

#[tokio::test]
async fn repository_supports_cancel_retry_and_delete() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let other_project_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into projects (id, name, description, created_at, updated_at)
        values ($1, $2, $3, now(), now())
        "#,
    )
    .bind(other_project_id)
    .bind("Other")
    .bind(Option::<String>::None)
    .execute(&pool)
    .await
    .expect("other project should insert");
    let repository = SourceRepository::new(pool.clone());

    let source = repository
        .create(
            project_id,
            SourceType::LocalFile,
            Some("/tmp/audio.wav"),
            None,
            None,
        )
        .await
        .expect("source should be created");
    repository
        .update_ingestion_status(source.id, "staged")
        .await
        .expect("staging should succeed");

    let cancelled = repository
        .cancel(project_id, source.id)
        .await
        .expect("cancel should succeed")
        .expect("source should exist");
    assert_eq!(cancelled.ingestion_status.as_str(), "failed");
    assert_eq!(cancelled.failure_reason.as_deref(), Some("cancelled"));

    let retried = repository
        .retry(project_id, source.id)
        .await
        .expect("retry should succeed")
        .expect("source should exist");
    assert_eq!(retried.ingestion_status.as_str(), "pending");
    assert_eq!(retried.failure_reason, None);

    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path, byte_size,
            ingestion_status, created_at, updated_at
        )
        values ($1, $2, $3, 'video', '/tmp/staged/source.bin', 12, 'ready', now(), now())
        "#,
    )
    .bind(media_asset_id)
    .bind(project_id)
    .bind(source.id)
    .execute(&pool)
    .await
    .expect("media asset should insert");
    sqlx::query(
        "insert into audio_assets (id, media_asset_id, project_id, processing_status, duration_seconds, audio_format, codec, sample_rate_hz, channels, byte_size, storage_path, created_at, updated_at) values ($1, $2, $3, 'validated', 1.0, 'flac', 'flac', 16000, 1, 100, '/data/audio/test.flac', now(), now())"
    )
    .bind(Uuid::new_v4())
    .bind(media_asset_id)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("audio asset should insert");

    let deleted = repository
        .delete(project_id, source.id)
        .await
        .expect("delete should succeed");
    assert!(deleted);
    assert!(repository
        .get_in_project(project_id, source.id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(repository.list(project_id).await.unwrap().len(), 0);

    let other_source = repository
        .create(
            other_project_id,
            SourceType::LocalFile,
            Some("/tmp/other.wav"),
            None,
            None,
        )
        .await
        .expect("other source should be created");
    assert!(!repository
        .delete(project_id, other_source.id)
        .await
        .expect("cross-project delete should not error"));
}
