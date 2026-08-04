mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::{
    config::AudioNormalizationProfile, db::Database, job::JobType, migrations, server::AppState,
    source::SourceType, storage::AudioStorageConfig,
};
use serde_json::Value;
use sqlx::PgPool;
use std::{env, path::PathBuf, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("listing_api_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPool::connect(&database_url).await.unwrap();
    sqlx::query(&format!("create schema {}", schema_name))
        .execute(&admin_pool)
        .await
        .unwrap();
    let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
    migrations::run(&schema_database_url).await.unwrap();
    PgPool::connect(&schema_database_url).await.unwrap()
}

async fn seed_project(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into projects (id, name, description, created_at, updated_at)
        values ($1, $2, null, now(), now())
        "#,
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn sources_and_jobs_can_be_listed_with_filters_and_pagination() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let other_project_id = seed_project(&pool, "Other").await;
    let database = Database::from_pool(pool.clone());

    let source_a = database
        .sources()
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/aaaa1111"),
            None,
        )
        .await
        .unwrap();
    let source_b = database
        .sources()
        .create(
            project_id,
            SourceType::LocalFile,
            Some("/tmp/audio.wav"),
            None,
        )
        .await
        .unwrap();
    database
        .sources()
        .update_ingestion_status(source_a.id, "staged")
        .await
        .unwrap();
    database
        .sources()
        .update_ingestion_status(source_b.id, "failed")
        .await
        .unwrap();
    database
        .sources()
        .create(
            other_project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/zzzz9999"),
            None,
        )
        .await
        .unwrap();

    let _job_a = database
        .jobs()
        .create(
            project_id,
            Some(source_a.id),
            JobType::MediaIngestion,
            1,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let job_b = database
        .jobs()
        .create(
            project_id,
            Some(source_b.id),
            JobType::AudioExtraction,
            10,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let failure_reason = "very long failure message ".repeat(20);
    sqlx::query(
        r#"
        update jobs
        set status = 'failed',
            failure_reason = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_b.id)
    .bind(failure_reason.as_str())
    .execute(&pool)
    .await
    .unwrap();
    database
        .jobs()
        .create(
            other_project_id,
            Some(Uuid::new_v4()),
            JobType::AudioExtraction,
            1,
            chrono::Utc::now(),
        )
        .await
        .unwrap();

    let app = backend::http::router(AppState {
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
    });

    let response = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{project_id}/sources?limit=1&offset=0&source_type=public_url&ingestion_status=staged"
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
    let sources: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(sources["items"].as_array().unwrap().len(), 1);
    assert_eq!(sources["items"][0]["id"], source_a.id.to_string());
    assert_eq!(sources["items"][0]["progress"], "staged");

    let response = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{project_id}/jobs?limit=10&offset=0&job_type=audio_extraction&status=failed"
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
    let jobs: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(jobs["items"].as_array().unwrap().len(), 1);
    assert_eq!(jobs["items"][0]["id"], job_b.id.to_string());
    assert!(jobs["items"][0]["failure_summary"].as_str().unwrap().len() <= 256);

    let response = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{other_project_id}/sources?limit=10&offset=0"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let other_sources: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(other_sources["items"].as_array().unwrap().len(), 1);

    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{project_id}/sources?limit=1&offset=1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let paged: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(paged["items"].as_array().unwrap().len(), 1);
    assert_eq!(paged["items"][0]["id"], source_b.id.to_string());
}

#[tokio::test]
async fn retry_endpoint_requeues_failed_jobs_and_is_project_scoped() {
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
        )
        .await
        .unwrap();
    let job = database
        .jobs()
        .create(
            project_id,
            Some(source.id),
            JobType::AudioExtraction,
            1,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    sqlx::query(
        r#"
        update jobs
        set status = 'failed',
            attempts = 1,
            failure_reason = 'temporary failure',
            lease_expires_at = now(),
            worker_id = 'worker-1',
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job.id)
    .execute(&pool)
    .await
    .unwrap();
    let other_source = database
        .sources()
        .create(
            other_project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/zzzz9999"),
            None,
        )
        .await
        .unwrap();
    let other_job = database
        .jobs()
        .create(
            other_project_id,
            Some(other_source.id),
            JobType::AudioExtraction,
            1,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    sqlx::query(
        r#"
        update jobs
        set status = 'failed',
            attempts = 1,
            failure_reason = 'temporary failure'
        where id = $1
        "#,
    )
    .bind(other_job.id)
    .execute(&pool)
    .await
    .unwrap();

    let app = backend::http::router(AppState {
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
    });

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/projects/{project_id}/jobs/{}/retry", job.id))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"available_at":"2026-08-04T00:00:00Z"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retried: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(retried["status"], "queued");
    assert_eq!(retried["lease_expires_at"], Value::Null);
    assert_eq!(retried["worker_id"], Value::Null);

    let repeated = app
        .clone()
        .oneshot(
            Request::post(format!("/projects/{project_id}/jobs/{}/retry", job.id))
                .header("content-type", "application/json")
                .body(Body::from(r#"{}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repeated.status(), StatusCode::OK);

    let forbidden = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/projects/{other_project_id}/jobs/{}/retry",
                job.id
            ))
            .header("content-type", "application/json")
            .body(Body::from(r#"{}"#))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn staging_capacity_endpoint_reports_guard_state() {
    support::clear_env();
    let pool = test_pool().await;
    let database = Database::from_pool(pool);
    let app = backend::http::router(AppState {
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
    });

    let response = app
        .oneshot(
            Request::get("/staging/capacity")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let capacity: Value = serde_json::from_slice(&body).unwrap();
    assert!(capacity["free_bytes"].as_u64().is_some());
    assert!(capacity["total_bytes"].as_u64().is_some());
}
