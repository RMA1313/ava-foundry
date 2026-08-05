mod support;

use backend::{
    job::{JobType, WorkerRepository},
    migrations,
};
use chrono::{Duration, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("worker_registry_test_{}", Uuid::new_v4().simple());
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
async fn worker_registration_is_idempotent_and_tracks_expiry() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = WorkerRepository::new(pool.clone());
    let expires_at = Utc::now() + Duration::minutes(5);

    let registered = repository
        .register(
            "worker-a",
            "1.0.0",
            &[JobType::MediaIngestion, JobType::AudioExtraction],
            expires_at,
        )
        .await
        .expect("worker should register");
    assert_eq!(registered.worker_id, "worker-a");
    assert_eq!(registered.capabilities.len(), 2);
    assert_eq!(registered.active_lease_count, 0);

    let heartbeat = repository
        .heartbeat("worker-a", Utc::now() + Duration::minutes(10))
        .await
        .expect("heartbeat should succeed")
        .expect("worker should exist");
    assert_eq!(heartbeat.worker_id, "worker-a");
    assert!(heartbeat.expires_at > registered.expires_at);

    let missing = repository
        .heartbeat("worker-missing", Utc::now() + Duration::minutes(10))
        .await
        .expect("heartbeat should succeed");
    assert!(missing.is_none());

    let row = sqlx::query_scalar::<_, String>(
        "select worker_id from worker_registry where expires_at > now() order by worker_id asc limit 1",
    )
    .fetch_one(&pool)
    .await
    .expect("worker should be queryable");
    assert_eq!(row, "worker-a");
}
