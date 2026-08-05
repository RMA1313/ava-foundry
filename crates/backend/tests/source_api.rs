mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::{
    config::AudioNormalizationProfile, db::Database, migrations, server::AppState,
    source::SourceType, storage::AudioStorageConfig,
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, path::PathBuf, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("source_api_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .unwrap();
    sqlx::query(&format!("create schema {}", schema_name))
        .execute(&admin_pool)
        .await
        .unwrap();
    let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
    migrations::run(&schema_database_url).await.unwrap();
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&schema_database_url)
        .await
        .unwrap()
}

async fn seed_project(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "insert into projects (id, name, description, created_at, updated_at) values ($1, $2, null, now(), now())",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    id
}

fn app(database: Database) -> axum::Router {
    backend::http::router(AppState {
        database,
        staging_dir: PathBuf::from("/tmp"),
        audio_storage: AudioStorageConfig::new(
            PathBuf::from("/tmp/audio"),
            PathBuf::from("/tmp/audio-tmp"),
        ),
        audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
        staging_capacity_guard: Arc::new(backend::worker::StagingCapacityGuard::new(
            1,
            2,
            backend::worker::DefaultFilesystemMetrics,
        )),
    })
}

#[tokio::test]
async fn source_api_supports_project_scoped_crud_and_transitions() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let other_project_id = seed_project(&pool, "Other").await;
    let database = Database::from_pool(pool.clone());

    let source = database
        .sources()
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/abcd1234"),
            None,
            Some(r#"{"origin":"manual"}"#),
        )
        .await
        .unwrap();
    let other_source = database
        .sources()
        .create(
            other_project_id,
            SourceType::LocalFile,
            Some("/tmp/other.wav"),
            None,
            None,
        )
        .await
        .unwrap();
    database
        .sources()
        .update_ingestion_status(source.id, "staged")
        .await
        .unwrap();
    database
        .sources()
        .update_failure_state(other_source.id, Some("failed"))
        .await
        .unwrap();

    let app = app(database.clone());

    let response = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{project_id}/sources?limit=10&offset=0&search=abcd&sort=created_at&direction=asc"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["id"], source.id.to_string());
    assert_eq!(list["items"][0]["file_count"], 0);
    assert_eq!(list["items"][0]["byte_count"], 0);

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{project_id}/sources/{}", source.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail["source_config"], Value::Null);
    assert_eq!(detail["failure_reason"], Value::Null);

    let response = app
        .clone()
        .oneshot(
            Request::put(format!("/projects/{project_id}/sources/{}", source.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"source_config":"{\"recursive\":true}","import_metadata":"{\"origin\":\"api\"}","file_count":3,"byte_count":4096}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["file_count"], 3);
    assert_eq!(updated["byte_count"], 4096);

    let response = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/projects/{project_id}/sources/{}/cancel",
                source.id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let cancelled: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(cancelled["ingestion_status"], "failed");
    assert_eq!(cancelled["failure_reason"], "cancelled");

    let response = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/projects/{project_id}/sources/{}/retry",
                source.id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retried: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(retried["ingestion_status"], "pending");
    assert_eq!(retried["failure_reason"], Value::Null);

    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/projects/{project_id}/sources/{}", source.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(database
        .sources()
        .get_in_project(project_id, source.id)
        .await
        .unwrap()
        .is_none());

    let forbidden = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{other_project_id}/sources/{}",
                source.id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn url_import_rejects_private_network_targets() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let database = Database::from_pool(pool.clone());
    let app = app(database);

    let response = app
        .oneshot(
            Request::post(format!("/projects/{project_id}/sources/url"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"url":"http://127.0.0.1/secret"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "invalid_request");
}
