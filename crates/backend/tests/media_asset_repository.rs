mod support;

use backend::media_asset::MediaAssetUpsert;
use backend::{media_asset::MediaAssetRepository, migrations};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("media_asset_repo_test_{}", Uuid::new_v4().simple());
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
    sqlx::query("truncate table media_assets, sources, projects restart identity cascade")
        .execute(&pool)
        .await
        .expect("failed to clear media tables");
    pool
}

async fn seed_project_and_source(pool: &PgPool) -> (Uuid, Uuid) {
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

    (project_id, source_id)
}

#[tokio::test]
async fn repository_round_trip_and_idempotency_work() {
    support::clear_env();
    let pool = test_pool().await;
    let (project_id, source_id) = seed_project_and_source(&pool).await;
    let repository = MediaAssetRepository::new(pool.clone());

    let first = repository
        .upsert_ingested(
            &pool,
            MediaAssetUpsert {
                project_id,
                source_id,
                media_kind: "video".to_string(),
                staged_path: "/tmp/staged/source.bin".to_string(),
                original_filename: Some("source.mp4".to_string()),
                byte_size: 12,
                ingestion_status: "ready".to_string(),
            },
        )
        .await
        .expect("media asset should be created");

    let second = repository
        .upsert_ingested(
            &pool,
            MediaAssetUpsert {
                project_id,
                source_id,
                media_kind: "video".to_string(),
                staged_path: "/tmp/staged/source.bin".to_string(),
                original_filename: Some("source.mp4".to_string()),
                byte_size: 12,
                ingestion_status: "ready".to_string(),
            },
        )
        .await
        .expect("media asset should be upserted");

    assert_eq!(first.id, second.id);
    assert_eq!(second.project_id, project_id);
    assert_eq!(second.source_id, source_id);

    let fetched = repository
        .get_by_source(source_id)
        .await
        .expect("media asset lookup should succeed")
        .expect("media asset should exist");
    assert_eq!(fetched.id, first.id);
    assert_eq!(fetched.staged_path, "/tmp/staged/source.bin");
}

#[tokio::test]
async fn migration_creates_media_assets_table_with_source_link_and_unique_constraint() {
    support::clear_env();
    let pool = test_pool().await;
    let (project_id, source_id) = seed_project_and_source(&pool).await;

    sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(source_id)
    .bind("audio")
    .bind("/tmp/staged/source.wav")
    .bind(Some("source.wav"))
    .bind(42_i64)
    .bind("ready")
    .execute(&pool)
    .await
    .expect("media asset should insert through migration schema");

    let duplicate = sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(source_id)
    .bind("audio")
    .bind("/tmp/staged/source-duplicate.wav")
    .bind(Some("source.wav"))
    .bind(99_i64)
    .bind("ready")
    .execute(&pool)
    .await;
    assert!(duplicate.is_err());

    let orphan = sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind(Uuid::new_v4())
    .bind("audio")
    .bind("/tmp/staged/orphan.wav")
    .bind(Option::<String>::None)
    .bind(5_i64)
    .bind("ready")
    .execute(&pool)
    .await;
    assert!(orphan.is_err());
}
