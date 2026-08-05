mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::{
    config::AudioNormalizationProfile, db::Database, migrations, server::AppState,
    storage::AudioStorageConfig,
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("upload_api_test_{}", Uuid::new_v4().simple());
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

async fn seed_project(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "insert into projects (id, name, description, created_at, updated_at) values ($1, $2, null, now(), now())",
    )
    .bind(id)
    .bind("Research")
    .execute(pool)
    .await
    .unwrap();
    id
}

fn app(database: Database, staging_dir: PathBuf) -> axum::Router {
    backend::http::router(AppState {
        database,
        staging_dir,
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

fn multipart_body(boundary: &str, parts: &[(&str, &str, &str, &str)]) -> String {
    let mut body = String::new();
    for (name, filename, content_type, content) in parts {
        body.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n{content}\r\n"
        ));
    }
    body.push_str(&format!("--{boundary}--\r\n"));
    body
}

#[tokio::test]
async fn upload_api_streams_multiple_files_and_creates_source_and_job() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let database = Database::from_pool(pool.clone());
    let temp_dir = TempDir::new().unwrap();
    let app = app(database.clone(), temp_dir.path().to_path_buf());
    let boundary = "X-BOUNDARY";
    let body = multipart_body(
        boundary,
        &[
            ("file", "alpha.wav", "audio/wav", "alpha"),
            ("file", "beta.mp3", "audio/mpeg", "beta"),
        ],
    );

    let response = app
        .oneshot(
            Request::post(format!("/projects/{project_id}/uploads/media"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["total_files"], 2);
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
    assert_eq!(json["items"][0]["byte_size"], 5);
    assert_eq!(json["items"][1]["byte_size"], 4);

    let source_count = sqlx::query_scalar::<_, i64>("select count(*) from sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    let job_count = sqlx::query_scalar::<_, i64>("select count(*) from jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(source_count, 2);
    assert_eq!(job_count, 2);
}

#[tokio::test]
async fn upload_api_rejects_path_traversal_filenames() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let database = Database::from_pool(pool.clone());
    let temp_dir = TempDir::new().unwrap();
    let app = app(database, temp_dir.path().to_path_buf());
    let boundary = "X-BOUNDARY";
    let body = multipart_body(boundary, &[("file", "../secret.wav", "audio/wav", "alpha")]);

    let response = app
        .oneshot(
            Request::post(format!("/projects/{project_id}/uploads/media"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
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
