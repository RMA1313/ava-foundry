mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use backend::config::AudioNormalizationProfile;
use backend::storage::AudioStorageConfig;
use backend::worker::{DefaultFilesystemMetrics, StagingCapacityGuard};
use backend::{db::Database, migrations, server::AppState};
use serde_json::Value;
use sqlx::PgPool;
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = std::env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("project_api_test_{}", Uuid::new_v4().simple());
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

async fn router() -> Router {
    support::clear_env();
    let pool = test_pool().await;
    let state = AppState {
        database: Database::from_pool(pool),
        staging_dir: PathBuf::from("/tmp"),
        audio_storage: AudioStorageConfig::new(
            PathBuf::from("/tmp/audio"),
            PathBuf::from("/tmp/audio-tmp"),
        ),
        audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
        staging_capacity_guard: Arc::new(StagingCapacityGuard::new(1, 2, DefaultFilesystemMetrics)),
    };
    backend::http::router(state)
}

#[tokio::test]
async fn project_lifecycle_works_through_http() {
    let app = router().await;

    let response = app
        .clone()
        .oneshot(
            Request::post("/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"  Research  ","description":"  Audio collection  "}"#,
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let created: Value = serde_json::from_slice(&body).expect("valid json");
    let id = created["id"].as_str().expect("id should be present");

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::get("/projects?limit=10&offset=0")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let listed: Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(
        listed["items"]
            .as_array()
            .expect("items should be array")
            .len(),
        1
    );

    let response = app
        .clone()
        .oneshot(
            Request::put(format!("/projects/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Updated","description":null}"#))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn invalid_project_requests_return_deterministic_errors() {
    let app = router().await;

    let response = app
        .clone()
        .oneshot(
            Request::post("/projects")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"   ","description":null}"#))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{}", Uuid::new_v4()))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
