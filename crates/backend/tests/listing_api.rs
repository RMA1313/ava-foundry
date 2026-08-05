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
    let schema_name = format!("listing_api_test_{}", Uuid::new_v4().simple());
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
async fn audio_assets_can_be_listed_and_read_with_project_isolation() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let other_project_id = seed_project(&pool, "Other").await;
    let database = Database::from_pool(pool.clone());

    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
        values ($1, $2, $3, $4, $5, null, $6, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(project_id)
    .bind("public_url")
    .bind(Some("https://youtu.be/aaaa1111"))
    .bind("url:youtube:aaaa1111")
    .bind("staged")
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, inspection_status,
            extraction_status, extraction_progress, extraction_tmp_path, extracted_at,
            container_format, duration_seconds, codecs, sample_rate_hz, channels,
            inspected_at, created_at, updated_at
        )
        values (
            '{media_asset_id}',
            '{project_id}',
            (select id from sources where project_id = '{project_id}' limit 1),
            'video',
            '/tmp/staged/source.bin',
            'source.mp4',
            12,
            'ready',
            'inspectable',
            'extracted',
            'running',
            null,
            null,
            'mp4',
            12.5,
            'aac',
            16000,
            1,
            null,
            now(),
            now()
        )
        "
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into audio_assets (
            id, media_asset_id, project_id, processing_status, duration_seconds,
            normalization_profile, validation_status, validation_error, validated_at,
            audio_format, codec, sample_rate_hz, channels, byte_size, storage_path,
            created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, null, now(), $8, $9, $10, $11, $12, $13, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(media_asset_id)
    .bind(project_id)
    .bind("validated")
    .bind(12.5_f64)
    .bind(Some("flac:flac:16000:1"))
    .bind(Some("validated"))
    .bind("flac")
    .bind("flac")
    .bind(16000_i32)
    .bind(1_i32)
    .bind(1024_i64)
    .bind("/data/audio/audio-assets/asset.flac")
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into sources (id, project_id, source_type, original_url, normalized_identity, import_metadata, ingestion_status, created_at, updated_at)
        values ($1, $2, $3, $4, $5, null, $6, now(), now())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(other_project_id)
    .bind("public_url")
    .bind(Some("https://youtu.be/zzzz9999"))
    .bind("url:youtube:zzzz9999")
    .bind("staged")
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
            Request::get(format!(
                "/projects/{project_id}/audio-assets?limit=1&offset=0&processing_status=validated"
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
    let assets: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(assets["items"].as_array().unwrap().len(), 1);
    assert_eq!(assets["items"][0]["project_id"], project_id.to_string());
    assert_eq!(
        assets["items"][0]["media_asset_id"],
        media_asset_id.to_string()
    );
    assert_eq!(assets["items"][0]["source_id"].as_str().unwrap().len(), 36);
    assert_eq!(assets["items"][0]["processing_status"], "validated");

    let asset_id = assets["items"][0]["id"].as_str().unwrap().to_string();
    let detail = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{project_id}/audio-assets/{asset_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let body = axum::body::to_bytes(detail.into_body(), usize::MAX)
        .await
        .unwrap();
    let asset: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(asset["storage_path"], "/data/audio/audio-assets/asset.flac");
    assert_eq!(asset["validation_status"], "validated");

    let missing = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{other_project_id}/audio-assets/{asset_id}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let paged = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{project_id}/audio-assets?limit=1&offset=1"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paged.status(), StatusCode::OK);
    let body = axum::body::to_bytes(paged.into_body(), usize::MAX)
        .await
        .unwrap();
    let page: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn media_assets_can_be_listed_with_filters_and_project_isolation() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool, "Research").await;
    let other_project_id = seed_project(&pool, "Other").await;
    let database = Database::from_pool(pool.clone());

    let source_record_id = database
        .sources()
        .create(
            project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/aaaa1111"),
            None,
            None,
        )
        .await
        .unwrap();
    let other_source_id = database
        .sources()
        .create(
            other_project_id,
            SourceType::PublicUrl,
            Some("https://youtu.be/zzzz9999"),
            None,
            None,
        )
        .await
        .unwrap();
    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, inspection_status,
            extraction_status, extraction_progress, extraction_tmp_path, extracted_at,
            container_format, duration_seconds, codecs, sample_rate_hz, channels,
            inspected_at, created_at, updated_at
        )
        values (
            $1, $2, $3, 'video', '/tmp/staged/source.bin',
            'source.mp4', 12, 'ready', 'inspectable',
            'extracted', 'running', null, null,
            'mp4', 12.5, 'aac', 16000, 1,
            null, now(), now()
        )
        "#,
    )
    .bind(media_asset_id)
    .bind(project_id)
    .bind(source_record_id.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status, inspection_status,
            extraction_status, extraction_progress, extraction_tmp_path, extracted_at,
            container_format, duration_seconds, codecs, sample_rate_hz, channels,
            inspected_at, created_at, updated_at
        )
        values (
            $1, $2, $3, 'audio', '/tmp/staged/other.bin',
            'other.mp3', 34, 'pending', null,
            null, null, null, null,
            null, null, null, null, null,
            null, now(), now()
        )
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(other_project_id)
    .bind(other_source_id.id)
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
        .oneshot(Request::get(format!("/projects/{project_id}/media-assets?limit=1&offset=0&ingestion_status=ready&inspection_status=inspectable&extraction_status=extracted&media_kind=video")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let assets: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(assets["items"].as_array().unwrap().len(), 1);
    assert_eq!(assets["items"][0]["id"], media_asset_id.to_string());
    assert_eq!(
        assets["items"][0]["source_id"],
        source_record_id.id.to_string()
    );
    assert_eq!(assets["items"][0]["media_kind"], "video");

    let other = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/projects/{other_project_id}/media-assets?limit=10&offset=0"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(other.into_body(), usize::MAX)
        .await
        .unwrap();
    let other_assets: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(other_assets["items"].as_array().unwrap().len(), 1);
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
