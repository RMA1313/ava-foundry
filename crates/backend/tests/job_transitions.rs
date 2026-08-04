mod support;

use backend::{
    job::{JobRepository, JobStatus, JobType},
    migrations,
};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("job_transition_test_{}", Uuid::new_v4().simple());
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
    PgPool::connect(&schema_database_url)
        .await
        .expect("failed to connect to schema-scoped test database")
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

async fn seed_source(pool: &PgPool, project_id: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
        values ($1, $2, $3, $4, $5, $6, $7, now(), now())
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind("public_url")
    .bind(Some("https://youtu.be/abcd1234"))
    .bind("url:youtube:abcd1234")
    .bind(Option::<String>::None)
    .bind("pending")
    .execute(pool)
    .await
    .expect("source should insert");
    id
}

#[tokio::test]
async fn claim_respects_priority_and_is_atomic() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let source_id = seed_source(&pool, project_id).await;
    let repository = JobRepository::new(pool.clone());

    let lower = repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioExtraction,
            1,
            Utc::now(),
        )
        .await
        .expect("job should be created");
    let higher = repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioValidation,
            10,
            Utc::now(),
        )
        .await
        .expect("job should be created");

    let claimed = repository
        .claim_next("worker-a", Utc::now() + Duration::minutes(5))
        .await
        .expect("claim should succeed")
        .expect("a job should be claimable");
    assert_eq!(claimed.id, higher.id);
    assert_eq!(claimed.status, JobStatus::Running);

    let next = repository
        .claim_next("worker-b", Utc::now() + Duration::minutes(5))
        .await
        .expect("claim should succeed")
        .expect("a second job should be claimable");
    assert_eq!(next.id, lower.id);
}

#[tokio::test]
async fn terminal_transitions_are_idempotent() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let source_id = seed_source(&pool, project_id).await;
    let repository = JobRepository::new(pool.clone());

    let created = repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioExtraction,
            1,
            Utc::now(),
        )
        .await
        .expect("job should be created");
    let claimed = repository
        .claim_next("worker-a", Utc::now() + Duration::minutes(5))
        .await
        .expect("claim should succeed")
        .expect("a job should be claimable");
    assert_eq!(claimed.id, created.id);

    let completed = repository
        .complete(created.id)
        .await
        .expect("completion should succeed")
        .expect("job should exist");
    assert_eq!(completed.status, JobStatus::Completed);

    let repeated = repository
        .complete(created.id)
        .await
        .expect("repeated completion should succeed")
        .expect("job should exist");
    assert_eq!(repeated.status, JobStatus::Completed);
}

#[tokio::test]
async fn retry_scheduling_is_bounded() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let source_id = seed_source(&pool, project_id).await;
    let repository = JobRepository::new(pool.clone());

    let created = repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioExtraction,
            1,
            Utc::now(),
        )
        .await
        .expect("job should be created");
    let claimed = repository
        .claim_next("worker-a", Utc::now() + Duration::minutes(5))
        .await
        .expect("claim should succeed")
        .expect("a job should be claimable");
    assert_eq!(claimed.id, created.id);

    let retried = repository
        .schedule_retry(
            created.id,
            "temporary failure",
            Utc::now() + Duration::minutes(10),
            3,
        )
        .await
        .expect("retry should succeed")
        .expect("job should exist");
    assert_eq!(retried.status, JobStatus::Queued);
    assert_eq!(retried.attempts, 1);

    let exhausted = repository
        .schedule_retry(
            created.id,
            "temporary failure",
            Utc::now() + Duration::minutes(10),
            1,
        )
        .await
        .expect("retry should succeed")
        .expect("job should exist");
    assert_eq!(exhausted.status, JobStatus::Failed);
}

#[tokio::test]
async fn lease_recovery_requeues_expired_jobs() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let source_id = seed_source(&pool, project_id).await;
    let repository = JobRepository::new(pool.clone());

    let created = repository
        .create(
            project_id,
            Some(source_id),
            JobType::AudioExtraction,
            1,
            Utc::now(),
        )
        .await
        .expect("job should be created");
    let claimed = repository
        .claim_next("worker-a", Utc::now() - Duration::minutes(1))
        .await
        .expect("claim should succeed")
        .expect("a job should be claimable");
    assert_eq!(claimed.id, created.id);

    let recovered = repository
        .recover_expired_leases(Utc::now())
        .await
        .expect("recovery should succeed");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, JobStatus::Queued);
}
