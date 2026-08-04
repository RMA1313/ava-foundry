mod support;

use backend::{migrations, project::ProjectRepository};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("project_repo_test_{}", Uuid::new_v4().simple());
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
        .update(created.id, "Updated", None)
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
