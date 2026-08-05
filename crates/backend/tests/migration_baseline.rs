mod support;

use backend::migrations;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("migration_baseline_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to test database");
    sqlx::query(&format!("create schema {}", schema_name))
        .execute(&admin_pool)
        .await
        .expect("failed to create test schema");
    drop(admin_pool);
    let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
    migrations::run(&schema_database_url)
        .await
        .expect("failed to run migrations");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&schema_database_url)
        .await
        .expect("failed to connect to schema-scoped test database")
}

#[tokio::test]
async fn clean_database_reaches_the_latest_schema() {
    support::clear_env();
    let pool = test_pool().await;
    let schema_name = sqlx::query_scalar::<_, String>("select current_schema()")
        .fetch_one(&pool)
        .await
        .expect("current schema should be available");

    let media_columns = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from information_schema.columns
        where table_schema = $1
          and table_name = 'media_assets'
          and column_name in (
              'inspection_status',
              'container_format',
              'duration_seconds',
              'codecs',
              'sample_rate_hz',
              'channels',
              'inspected_at'
          )
        "#,
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await
    .expect("column inspection should succeed");
    assert_eq!(media_columns, 7);

    let job_columns = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from information_schema.columns
        where table_schema = $1
          and table_name = 'jobs'
          and column_name in (
              'parent_job_id',
              'batch_id',
              'max_attempts',
              'retry_count',
              'progress_current',
              'progress_total',
              'result_payload',
              'failure_payload',
              'cancel_requested_at',
              'cancelled_at'
          )
        "#,
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await
    .expect("job column inspection should succeed");
    assert_eq!(job_columns, 10);

    let worker_tables = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from information_schema.tables
        where table_schema = $1
          and table_name = 'worker_registry'
        "#,
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await
    .expect("worker table inspection should succeed");
    assert_eq!(worker_tables, 1);

    let fingerprint_tables = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)
        from information_schema.tables
        where table_schema = $1
          and table_name in (
            'audio_fingerprints',
            'duplicate_groups',
            'duplicate_group_memberships',
            'audio_duplicate_decisions'
        )
        "#,
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await
    .expect("table inspection should succeed");
    assert_eq!(fingerprint_tables, 4);
}
