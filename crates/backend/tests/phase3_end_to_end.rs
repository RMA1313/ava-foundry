mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::{
    audio_asset::{AudioAssetRepository, AudioAssetUpsert},
    config::AudioNormalizationProfile,
    db::Database,
    job::JobType,
    migrations,
    server::AppState,
    storage::AudioStorageConfig,
    worker::{
        fingerprint_handler, DefaultFilesystemMetrics, JobHandler, JobOutcome, StagingCapacityGuard,
    },
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{env, fs, path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("phase3_e2e_test_{}", Uuid::new_v4().simple());
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
    let project_id = Uuid::new_v4();
    sqlx::query("insert into projects (id, name, description, created_at, updated_at) values ($1, $2, null, now(), now())")
        .bind(project_id)
        .bind("Research")
        .execute(pool)
        .await
        .unwrap();
    project_id
}

async fn seed_finalized_audio_asset(
    pool: &PgPool,
    project_id: Uuid,
    media_suffix: &str,
    duration_seconds: f64,
    storage_path: &str,
) -> Uuid {
    let source_id = Uuid::new_v4();
    sqlx::query(
        "insert into sources (id, project_id, source_type, original_url, normalized_identity, ingestion_status, created_at, updated_at) values ($1, $2, $3, $4, $5, $6, now(), now())",
    )
    .bind(source_id)
    .bind(project_id)
    .bind("public_url")
    .bind(Some(format!("https://example.com/{media_suffix}")))
    .bind(format!("url:example:{media_suffix}"))
    .bind("completed")
    .execute(pool)
    .await
    .unwrap();

    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        "insert into media_assets (id, project_id, source_id, media_kind, staged_path, byte_size, ingestion_status, created_at, updated_at) values ($1, $2, $3, $4, $5, $6, $7, now(), now())",
    )
    .bind(media_asset_id)
    .bind(project_id)
    .bind(source_id)
    .bind("video")
    .bind(format!("/tmp/staged/{media_suffix}.bin"))
    .bind(1_i64)
    .bind("ready")
    .execute(pool)
    .await
    .unwrap();

    let audio_asset_id = Uuid::new_v4();
    AudioAssetRepository::new(pool.clone())
        .upsert(
            pool,
            AudioAssetUpsert {
                id: audio_asset_id,
                media_asset_id,
                project_id,
                processing_status: "validated".to_string(),
                normalization_profile: Some("flac:flac:16000:1".to_string()),
                validation_status: Some("validated".to_string()),
                validation_error: None,
                validated_at: Some(chrono::Utc::now()),
                duration_seconds,
                audio_format: "flac".to_string(),
                codec: "flac".to_string(),
                sample_rate_hz: 16000,
                channels: 1,
                byte_size: 1024,
                storage_path: storage_path.to_string(),
            },
        )
        .await
        .unwrap();
    audio_asset_id
}

fn fake_fingerprint(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("fake-fingerprint");
    fs::write(
        &path,
        "#!/bin/sh\noutput=\"$2\"\ncat <<'EOF' > \"$output\"\nfingerprint\nEOF\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    path
}

async fn execute_job(handler: &impl JobHandler, state: AppState, job_id: Uuid) {
    let job = state.database.jobs().get(job_id).await.unwrap().unwrap();
    match handler.handle(job.clone(), state.clone()).await.unwrap() {
        JobOutcome::Completed => {}
        outcome => panic!("unexpected job outcome: {outcome:?}"),
    }
}

#[tokio::test]
async fn phase3_pipeline_is_deterministic_restart_safe_and_api_visible() {
    support::clear_env();
    let pool = test_pool().await;
    let temp_dir = TempDir::new().unwrap();
    let audio_root = temp_dir.path().join("audio");
    let audio_tmp_root = temp_dir.path().join("audio-tmp");
    fs::create_dir_all(&audio_root).unwrap();
    fs::create_dir_all(&audio_tmp_root).unwrap();
    let database = Database::from_pool(pool.clone());
    let project_id = seed_project(&pool).await;
    let audio_one = seed_finalized_audio_asset(
        &pool,
        project_id,
        "one",
        3.0,
        "/tmp/audio/audio-assets/one.flac",
    )
    .await;
    let audio_two = seed_finalized_audio_asset(
        &pool,
        project_id,
        "two",
        3.0,
        "/tmp/audio/audio-assets/two.flac",
    )
    .await;
    let audio_three = seed_finalized_audio_asset(
        &pool,
        project_id,
        "three",
        8.0,
        "/tmp/audio/audio-assets/three.flac",
    )
    .await;
    let fingerprint_executable = fake_fingerprint(&temp_dir);
    let state = AppState {
        database: database.clone(),
        staging_dir: temp_dir.path().to_path_buf(),
        audio_storage: AudioStorageConfig::new(audio_root.clone(), audio_tmp_root.clone()),
        audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
        staging_capacity_guard: Arc::new(StagingCapacityGuard::new(1, 2, DefaultFilesystemMetrics)),
    };

    let job_one = database
        .jobs()
        .create(
            project_id,
            Some(audio_one),
            JobType::DuplicateDetection,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let job_two = database
        .jobs()
        .create(
            project_id,
            Some(audio_two),
            JobType::DuplicateDetection,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let job_three = database
        .jobs()
        .create(
            project_id,
            Some(audio_three),
            JobType::DuplicateDetection,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();

    std::env::set_var("INPUT_FILE", "/tmp/audio/audio-assets/one.flac");
    execute_job(
        &fingerprint_handler(&fingerprint_executable),
        state.clone(),
        job_one.id,
    )
    .await;
    std::env::set_var("INPUT_FILE", "/tmp/audio/audio-assets/two.flac");
    execute_job(
        &fingerprint_handler(&fingerprint_executable),
        state.clone(),
        job_two.id,
    )
    .await;
    std::env::set_var("INPUT_FILE", "/tmp/audio/audio-assets/three.flac");
    execute_job(
        &fingerprint_handler(&fingerprint_executable),
        state.clone(),
        job_three.id,
    )
    .await;

    let fingerprints = database
        .audio_fingerprints()
        .list_searchable_index("local-fingerprint", "1")
        .await
        .unwrap();
    assert_eq!(fingerprints.len(), 3);

    let decisions = sqlx::query_scalar::<_, i64>("select count(*) from audio_duplicate_decisions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(decisions, 1);
    let groups = sqlx::query_scalar::<_, i64>("select count(*) from duplicate_groups")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(groups, 1);
    let canonical = sqlx::query_scalar::<_, Option<Uuid>>(
        "select canonical_audio_asset_id from duplicate_groups limit 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let expected_canonical = if audio_one < audio_two {
        audio_one
    } else {
        audio_two
    };
    assert_eq!(canonical, Some(expected_canonical));

    sqlx::query("update duplicate_groups set canonical_audio_asset_id = null where id = (select id from duplicate_groups limit 1)")
        .execute(&pool)
        .await
        .unwrap();
    let recovered = database
        .duplicates()
        .recover_canonical_audio_assets()
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].1, expected_canonical);

    let app = backend::http::router(state.clone());
    let duplicate_response = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{project_id}/duplicates"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_response.status(), StatusCode::OK);
    let duplicate_body = axum::body::to_bytes(duplicate_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let duplicate_json: Value = serde_json::from_slice(&duplicate_body).unwrap();
    assert_eq!(duplicate_json["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        duplicate_json["items"][0]["decisions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        duplicate_json["items"][0]["canonical_audio_asset_id"],
        expected_canonical.to_string()
    );

    let metrics_response = app
        .clone()
        .oneshot(
            Request::get(format!("/projects/{project_id}/metrics/summary"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metrics_response.status(), StatusCode::OK);
    let metrics_body = axum::body::to_bytes(metrics_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let metrics: Value = serde_json::from_slice(&metrics_body).unwrap();
    assert_eq!(metrics["deduplication"]["candidate_count"], 1);
    assert_eq!(metrics["deduplication"]["match_rate"], 1.0);
    assert_eq!(metrics["deduplication"]["processing_failures"], 0);
    assert_eq!(metrics["deduplication"]["unique_duration_seconds"], 11.0);
    assert_eq!(
        metrics["deduplication"]["duplicate_ratio"],
        0.21428571428571427
    );
    assert!(metrics["deduplication"]["throughput"].as_f64().unwrap() >= 0.0);
    assert_eq!(metrics["stage_runs"].as_array().unwrap().len(), 3);

    let repeated = database
        .duplicates()
        .recover_canonical_audio_assets()
        .await
        .unwrap();
    assert!(repeated.is_empty());
}
