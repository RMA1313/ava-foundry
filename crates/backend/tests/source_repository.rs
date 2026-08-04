mod support;

use backend::{
    migrations,
    source::{SourceRepository, SourceType},
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
    let schema_name = format!("source_repo_test_{}", Uuid::new_v4().simple());
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
        )
        .await
        .expect("source should be created");
    let second = repository
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://www.youtube.com/watch?v=abcd1234"),
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
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("violates foreign key constraint"));
}
