mod support;

use backend::{migrations, project::ProjectRepository};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("project_repo_test_{}", Uuid::new_v4().simple());
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
    sqlx::query("truncate table projects restart identity cascade")
        .execute(&pool)
        .await
        .expect("failed to clear projects table");
    pool
}

#[tokio::test]
async fn repository_round_trip_works() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = ProjectRepository::new(pool.clone());

    let created = repository
        .create("  Research  ", Some("  Audio collection  "))
        .await
        .expect("project should be created");

    let fetched = repository
        .get(created.id)
        .await
        .expect("project lookup should succeed")
        .expect("project should exist");

    assert_eq!(fetched.name, "Research");
    assert_eq!(fetched.description.as_deref(), Some("Audio collection"));

    let listed = repository
        .list()
        .await
        .expect("project list should succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let updated = repository
        .update(created.id, "Updated", None, None)
        .await
        .expect("project update should succeed")
        .expect("project should exist");

    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.description, None);
    assert!(updated.updated_at >= created.updated_at);
}

#[tokio::test]
async fn repository_rejects_invalid_names() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = ProjectRepository::new(pool);

    let err = repository.create("   ", None).await.unwrap_err();
    assert!(err.to_string().contains("project name cannot be empty"));
}

#[tokio::test]
async fn repository_rejects_duplicate_names_case_insensitively() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = ProjectRepository::new(pool);

    repository
        .create("Research", None)
        .await
        .expect("first project should be created");
    let err = repository.create("research", None).await.unwrap_err();
    assert_eq!(err.to_string(), "project name already exists");
}

#[tokio::test]
async fn repository_rejects_duplicate_names_on_update_case_insensitively() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = ProjectRepository::new(pool);

    let first = repository
        .create("Research", None)
        .await
        .expect("first project should be created");
    let second = repository
        .create("Catalog", None)
        .await
        .expect("second project should be created");

    let err = repository
        .update(second.id, "research", None, None)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "project name already exists");

    let fetched = repository
        .get(first.id)
        .await
        .expect("lookup should succeed")
        .expect("first project should still exist");
    assert_eq!(fetched.name, "Research");
}

#[tokio::test]
async fn repository_deletes_project_content_transactionally() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = ProjectRepository::new(pool.clone());
    let project = repository.create("Research", None).await.expect("project");
    let source_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    sqlx::query("insert into sources (id, project_id, source_type, normalized_identity, ingestion_status, created_at, updated_at) values ($1, $2, 'local_file', 'file:///tmp/a', 'pending', now(), now())")
        .bind(source_id)
        .bind(project.id)
        .execute(&pool)
        .await
        .expect("source");
    sqlx::query("insert into jobs (id, project_id, source_id, job_type, status, priority, attempts, available_at, created_at, updated_at) values ($1, $2, $3, 'media_ingestion', 'queued', 1, 0, now(), now(), now())")
        .bind(job_id)
        .bind(project.id)
        .bind(source_id)
        .execute(&pool)
        .await
        .expect("job");

    assert!(repository.delete(project.id).await.expect("delete"));
    let remaining_sources =
        sqlx::query_scalar::<_, i64>("select count(*) from sources where project_id = $1")
            .bind(project.id)
            .fetch_one(&pool)
            .await
            .expect("count sources");
    let remaining_jobs =
        sqlx::query_scalar::<_, i64>("select count(*) from jobs where project_id = $1")
            .bind(project.id)
            .fetch_one(&pool)
            .await
            .expect("count jobs");
    assert_eq!(remaining_sources, 0);
    assert_eq!(remaining_jobs, 0);
}

#[tokio::test]
async fn repository_returns_none_for_missing_projects() {
    support::clear_env();
    let pool = test_pool().await;
    let repository = ProjectRepository::new(pool);

    let missing = repository
        .get(Uuid::new_v4())
        .await
        .expect("query should succeed");
    assert!(missing.is_none());
}
