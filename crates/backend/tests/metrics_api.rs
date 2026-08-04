mod support;

use axum::{body::Body, http::Request};
use backend::{
    db::Database, migrations, server::AppState, storage::AudioStorageConfig,
    worker::DefaultFilesystemMetrics, worker::StagingCapacityGuard,
};
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
    let schema_name = format!("metrics_api_test_{}", Uuid::new_v4().simple());
    let admin_pool = PgPool::connect(&database_url).await.unwrap();
    sqlx::query(&format!("create schema {}", schema_name))
        .execute(&admin_pool)
        .await
        .unwrap();
    let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
    migrations::run(&schema_database_url).await.unwrap();
    PgPool::connect(&schema_database_url).await.unwrap()
}

#[tokio::test]
async fn summary_endpoint_returns_stage_runs() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = Uuid::new_v4();
    sqlx::query("insert into projects (id, name, description, created_at, updated_at) values ($1, $2, null, now(), now())")
        .bind(project_id)
        .bind("Research")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("insert into stage_runs (id, project_id, media_asset_id, audio_asset_id, job_id, stage_name, status, queue_wait_ms, wall_time_ms, input_bytes, output_bytes, input_duration_seconds, throughput, failure_reason, cpu_percent, memory_bytes, created_at, updated_at) values ($1, $2, null, null, null, 'audio_validation', 'completed', 10, 20, 100, 200, 12.5, 0.625, null, null, null, now(), now())")
        .bind(Uuid::new_v4())
        .bind(project_id)
        .execute(&pool)
        .await
        .unwrap();
    let app = backend::http::router(AppState {
        database: Database::from_pool(pool.clone()),
        staging_dir: PathBuf::from("/tmp"),
        audio_storage: AudioStorageConfig::new(
            PathBuf::from("/tmp/audio"),
            PathBuf::from("/tmp/audio-tmp"),
        ),
        audio_normalization_profile:
            backend::config::AudioNormalizationProfile::default_flac_16khz_mono(),
        staging_capacity_guard: Arc::new(StagingCapacityGuard::new(1, 2, DefaultFilesystemMetrics)),
    });
    let response = app
        .oneshot(
            Request::get(format!("/projects/{project_id}/metrics/summary"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
}
