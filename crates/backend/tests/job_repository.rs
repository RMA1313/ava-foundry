mod support;

use backend::{
    job::{JobRepository, JobStatus, JobType},
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
    let schema_name = format!("job_repo_test_{}", Uuid::new_v4().simple());
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
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&schema_database_url)
        .await
        .expect("failed to connect to schema-scoped test database");
    sqlx::query("truncate table jobs restart identity cascade")
        .execute(&pool)
        .await
        .expect("failed to clear jobs table");
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
    let repository = JobRepository::new(pool.clone());
    let available_at = Utc::now();

    let created = repository
        .create(project_id, None, JobType::AudioExtraction, 5, available_at)
        .await
        .expect("job should be created");

    assert_eq!(created.project_id, project_id);
    assert_eq!(created.job_type, JobType::AudioExtraction);
    assert_eq!(created.status, JobStatus::Queued);
    assert_eq!(created.priority, 5);
    assert_eq!(created.attempts, 0);
    assert_eq!(created.max_attempts, 3);
    assert_eq!(created.retry_count, 0);
    assert_eq!(created.progress_current, 0);
    assert_eq!(created.progress_total, 0);

    let fetched = repository
        .get(created.id)
        .await
        .expect("job lookup should succeed")
        .expect("job should exist");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.max_attempts, 3);

    let listed = repository.list().await.expect("job list should succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[tokio::test]
async fn repository_rejects_invalid_priority() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = JobRepository::new(pool);

    let err = repository
        .create(project_id, None, JobType::AudioExtraction, 1001, Utc::now())
        .await
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("job priority must be between -1000 and 1000"));
}

#[tokio::test]
async fn repository_rejects_duplicate_logical_jobs() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = JobRepository::new(pool);

    let source_id = Uuid::new_v4();
    repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioExtraction,
            0,
            Utc::now(),
        )
        .await
        .expect("job should be created");
    let second = repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioExtraction,
            0,
            Utc::now(),
        )
        .await
        .expect("job should resolve deterministically");
    assert_eq!(
        second.id,
        repository
            .get_by_logical_key(project_id, Some(source_id), JobType::AudioExtraction)
            .await
            .unwrap()
            .unwrap()
            .id
    );
}

#[tokio::test]
async fn repository_orders_jobs_deterministically() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let repository = JobRepository::new(pool);
    let now = Utc::now();

    let later = now + Duration::seconds(10);
    let high = repository
        .create(project_id, None, JobType::AudioExtraction, 10, later)
        .await
        .expect("job should be created");
    let low = repository
        .create(
            project_id,
            Some(Uuid::new_v4()),
            JobType::AudioValidation,
            1,
            now,
        )
        .await
        .expect("job should be created");

    let listed = repository.list().await.expect("job list should succeed");
    assert_eq!(listed[0].id, high.id);
    assert_eq!(listed[1].id, low.id);
}
