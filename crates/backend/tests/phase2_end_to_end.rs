mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::{
    config::AudioNormalizationProfile,
    db::Database,
    job::{Job, JobStatus, JobType},
    migrations,
    server::AppState,
    storage::AudioStorageConfig,
    worker::{
        audio_extraction_handler, audio_validation_handler, media_inspection_handler,
        video_cleanup_handler, DefaultFilesystemMetrics, JobHandler, JobOutcome,
        StagingCapacityGuard, WorkerRuntime,
    },
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("phase2_e2e_test_{}", Uuid::new_v4().simple());
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

async fn seed_media_asset(pool: &PgPool, project_id: Uuid, staged_path: &Path) -> (Uuid, Uuid) {
    let source_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into sources (
            id, project_id, source_type, original_url, normalized_identity,
            import_metadata, ingestion_status, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, now(), now())
        "#,
    )
    .bind(source_id)
    .bind(project_id)
    .bind("local_file")
    .bind(Some("fixture://phase2/sample-video.mp4"))
    .bind("fixture:phase2:sample-video")
    .bind(Option::<String>::None)
    .bind("completed")
    .execute(pool)
    .await
    .unwrap();

    let media_asset_id = Uuid::new_v4();
    sqlx::query(
        r#"
        insert into media_assets (
            id, project_id, source_id, media_kind, staged_path,
            original_filename, byte_size, ingestion_status,
            inspection_status, container_format, duration_seconds, codecs,
            sample_rate_hz, channels, inspected_at, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, null, null, null, null, null, null, null, now(), now())
        "#,
    )
    .bind(media_asset_id)
    .bind(project_id)
    .bind(source_id)
    .bind("video")
    .bind(staged_path.to_string_lossy().to_string())
    .bind(Some("sample-video.mp4"))
    .bind(12_i64)
    .bind("ready")
    .execute(pool)
    .await
    .unwrap();

    (source_id, media_asset_id)
}

fn fake_executable(dir: &TempDir, name: &str, script: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    path
}

async fn execute_and_apply(
    handler: &impl JobHandler,
    state: AppState,
    job: Job,
) -> anyhow::Result<()> {
    match handler.handle(job.clone(), state.clone()).await? {
        JobOutcome::Completed => {
            if job.job_type != JobType::MediaIngestion && job.job_type != JobType::AudioValidation {
                let _ = state.database.jobs().complete(job.id).await?;
            }
        }
        JobOutcome::Failed { failure_reason } => {
            let _ = state.database.jobs().fail(job.id, &failure_reason).await?;
        }
        JobOutcome::Retry {
            failure_reason,
            retry_after,
        } => {
            let _ = state.database.jobs().fail(job.id, &failure_reason).await?;
            let _ = state
                .database
                .jobs()
                .schedule_retry(job.id, &failure_reason, retry_after, 3)
                .await?;
        }
    }
    Ok(())
}

fn app(state: AppState) -> axum::Router {
    backend::http::router(state)
}

#[tokio::test]
async fn phase2_fixture_media_completes_end_to_end_and_is_restart_safe() {
    support::clear_env();
    let pool = test_pool().await;
    let temp_dir = TempDir::new().unwrap();
    let staging_dir = temp_dir.path().join("staging");
    let audio_root = temp_dir.path().join("audio");
    let audio_tmp_root = temp_dir.path().join("audio-tmp");
    fs::create_dir_all(&staging_dir).unwrap();
    fs::create_dir_all(&audio_root).unwrap();
    fs::create_dir_all(&audio_tmp_root).unwrap();

    let fixture_video = staging_dir.join("phase2-fixture-video.mp4");
    fs::write(&fixture_video, b"fixture-video").unwrap();

    let project_id = seed_project(&pool, "Research").await;
    let (source_id, media_asset_id) = seed_media_asset(&pool, project_id, &fixture_video).await;
    let database = Database::from_pool(pool.clone());
    let state = AppState {
        database: database.clone(),
        staging_dir: staging_dir.clone(),
        audio_storage: AudioStorageConfig::new(audio_root.clone(), audio_tmp_root.clone()),
        audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
        staging_capacity_guard: Arc::new(StagingCapacityGuard::new(1, 2, DefaultFilesystemMetrics)),
    };

    let ffprobe_inspect = fake_executable(
        &temp_dir,
        "ffprobe-inspect",
        "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"matroska,webm\",\"duration\":\"12.5\"},\"streams\":[{\"codec_type\":\"video\",\"codec_name\":\"h264\"},{\"codec_type\":\"audio\",\"codec_name\":\"aac\",\"sample_rate\":\"48000\",\"channels\":2}]}\nJSON\n",
    );
    let ffmpeg_extract = fake_executable(
        &temp_dir,
        "ffmpeg-extract",
        "#!/bin/sh\nfor last; do :; done\ncp \"$INPUT_FILE\" \"$last\"\nprintf 'out_time_ms=1250\\nprogress=end\\n'\n",
    );
    let ffmpeg_normalize = fake_executable(
        &temp_dir,
        "ffmpeg-normalize",
        "#!/bin/sh\nfor last; do :; done\ncp \"$INPUT_FILE\" \"$last\"\nprintf 'out_time_ms=1250\\nprogress=end\\n'\n",
    );
    let ffprobe_validate = fake_executable(
        &temp_dir,
        "ffprobe-validate",
        "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"flac\",\"duration\":\"12.5\"},\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"flac\",\"sample_rate\":\"16000\",\"channels\":1}]}\nJSON\n",
    );

    let inspection_job = database
        .jobs()
        .create(
            project_id,
            Some(source_id),
            JobType::MediaInspection,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let _inspection_job = database
        .jobs()
        .claim_next(
            "phase2-inspection",
            chrono::Utc::now() + chrono::Duration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap();
    execute_and_apply(
        &media_inspection_handler(ffprobe_inspect),
        state.clone(),
        inspection_job,
    )
    .await
    .unwrap();
    let media_asset = database
        .media_assets()
        .get(media_asset_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        media_asset.inspection_status.as_deref(),
        Some("inspectable")
    );

    let extraction_job = database
        .jobs()
        .create(
            project_id,
            Some(media_asset_id),
            JobType::AudioExtraction,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let _extraction_job = database
        .jobs()
        .claim_next(
            "phase2-extraction",
            chrono::Utc::now() + chrono::Duration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap();
    std::env::set_var("INPUT_FILE", fixture_video.to_string_lossy().to_string());
    execute_and_apply(
        &audio_extraction_handler(ffmpeg_extract),
        state.clone(),
        extraction_job,
    )
    .await
    .unwrap();
    let media_asset = database
        .media_assets()
        .get(media_asset_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media_asset.extraction_status.as_deref(), Some("completed"));
    let extraction_tmp = media_asset.extraction_tmp_path.clone().unwrap();

    let validation_job = database
        .jobs()
        .create(
            project_id,
            Some(media_asset_id),
            JobType::AudioValidation,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let _validation_job = database
        .jobs()
        .claim_next(
            "phase2-validation",
            chrono::Utc::now() + chrono::Duration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap();
    std::env::set_var("INPUT_FILE", extraction_tmp.clone());
    execute_and_apply(
        &audio_validation_handler(ffmpeg_normalize, ffprobe_validate),
        state.clone(),
        validation_job,
    )
    .await
    .unwrap();

    let audio_asset = database
        .audio_assets()
        .get_by_media_asset(media_asset_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(audio_asset.processing_status, "validated");
    assert_eq!(audio_asset.validation_status.as_deref(), Some("validated"));
    assert!(Path::new(&audio_asset.storage_path).exists());

    let repeated_validation = audio_validation_handler(
        fake_executable(
            &temp_dir,
            "ffmpeg-normalize-repeat",
            "#!/bin/sh\nfor last; do :; done\ncp \"$INPUT_FILE\" \"$last\"\nprintf 'out_time_ms=1250\\nprogress=end\\n'\n",
        ),
        fake_executable(
            &temp_dir,
            "ffprobe-validate-repeat",
            "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"flac\",\"duration\":\"12.5\"},\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"flac\",\"sample_rate\":\"16000\",\"channels\":1}]}\nJSON\n",
        ),
    );
    let normalized_tmp = state
        .audio_storage
        .temporary_normalization_path(media_asset_id)
        .unwrap();
    let _ = fs::remove_file(&normalized_tmp);
    let _ = fs::remove_file(&audio_asset.storage_path);
    sqlx::query(
        r#"
        update media_assets
        set extraction_tmp_path = $2,
            extraction_status = 'completed',
            extraction_progress = 'validated',
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(media_asset_id)
    .bind(extraction_tmp.clone())
    .execute(&pool)
    .await
    .unwrap();
    let repeated_validation_job = database
        .jobs()
        .create(
            project_id,
            Some(media_asset_id),
            JobType::AudioValidation,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let repeated_validation_job = database
        .jobs()
        .claim_next(
            "phase2-validation-repeat",
            chrono::Utc::now() + chrono::Duration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap_or(repeated_validation_job);
    std::env::set_var("INPUT_FILE", extraction_tmp.clone());
    let repeated = repeated_validation
        .handle(repeated_validation_job, state.clone())
        .await
        .unwrap();
    assert!(matches!(repeated, JobOutcome::Completed));
    let audio_asset_again = database
        .audio_assets()
        .get_by_media_asset(media_asset_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(audio_asset_again.id, audio_asset.id);

    let cleanup_job = database
        .jobs()
        .create(
            project_id,
            Some(media_asset_id),
            JobType::VideoCleanup,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    execute_and_apply(&video_cleanup_handler(), state.clone(), cleanup_job)
        .await
        .unwrap();

    assert!(!fixture_video.exists());
    assert!(Path::new(&audio_asset.storage_path).exists());

    let audio_assets_response = app(state.clone())
        .oneshot(
            Request::get(format!("/projects/{project_id}/audio-assets"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(audio_assets_response.status(), StatusCode::OK);
    let audio_assets_body = axum::body::to_bytes(audio_assets_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let audio_assets: Value = serde_json::from_slice(&audio_assets_body).unwrap();
    assert_eq!(audio_assets["items"].as_array().unwrap().len(), 1);
    assert_eq!(audio_assets["items"][0]["id"], audio_asset.id.to_string());

    let metrics_response = app(state.clone())
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
    assert!(metrics["stage_runs"].as_array().unwrap().len() >= 3);
}

#[tokio::test]
async fn phase2_extraction_failure_is_retryable_and_recovery_keeps_finalized_audio() {
    support::clear_env();
    let pool = test_pool().await;
    let temp_dir = TempDir::new().unwrap();
    let staging_dir = temp_dir.path().join("staging");
    let audio_root = temp_dir.path().join("audio");
    let audio_tmp_root = temp_dir.path().join("audio-tmp");
    fs::create_dir_all(&staging_dir).unwrap();
    fs::create_dir_all(&audio_root).unwrap();
    fs::create_dir_all(&audio_tmp_root).unwrap();

    let fixture_video = staging_dir.join("retry-fixture-video.mp4");
    fs::write(&fixture_video, b"retry-video").unwrap();

    let project_id = seed_project(&pool, "Retry").await;
    let (source_id, media_asset_id) = seed_media_asset(&pool, project_id, &fixture_video).await;
    let database = Database::from_pool(pool.clone());
    let state = AppState {
        database: database.clone(),
        staging_dir: staging_dir.clone(),
        audio_storage: AudioStorageConfig::new(audio_root.clone(), audio_tmp_root.clone()),
        audio_normalization_profile: AudioNormalizationProfile::default_flac_16khz_mono(),
        staging_capacity_guard: Arc::new(StagingCapacityGuard::new(1, 2, DefaultFilesystemMetrics)),
    };

    let _ffprobe_inspect = fake_executable(
        &temp_dir,
        "retry-ffprobe-inspect",
        "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"matroska,webm\",\"duration\":\"12.5\"},\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"aac\",\"sample_rate\":\"48000\",\"channels\":2}]}\nJSON\n",
    );
    let ffmpeg_fail = fake_executable(
        &temp_dir,
        "retry-ffmpeg-fail",
        "#!/bin/sh\nfor last; do :; done\nprintf 'out_time_ms=1250\\nprogress=continue\\n'\nexit 1\n",
    );

    database
        .jobs()
        .create(
            project_id,
            Some(source_id),
            JobType::MediaInspection,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let inspection_job = database
        .jobs()
        .claim_next(
            "phase2-retry-inspection",
            chrono::Utc::now() + chrono::Duration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap();
    execute_and_apply(
        &media_inspection_handler(fake_executable(
            &temp_dir,
            "retry-ffprobe-inspect-2",
            "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"matroska,webm\",\"duration\":\"12.5\"},\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"aac\",\"sample_rate\":\"48000\",\"channels\":2}]}\nJSON\n",
        )),
        state.clone(),
        inspection_job,
    )
    .await
    .unwrap();

    std::env::set_var("INPUT_FILE", fixture_video.to_string_lossy().to_string());
    let _extraction_job = database
        .jobs()
        .create(
            project_id,
            Some(media_asset_id),
            JobType::AudioExtraction,
            0,
            chrono::Utc::now(),
        )
        .await
        .unwrap();
    let extraction_job = database
        .jobs()
        .claim_next(
            "phase2-retry-extraction",
            chrono::Utc::now() + chrono::Duration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap();
    execute_and_apply(
        &audio_extraction_handler(ffmpeg_fail),
        state.clone(),
        extraction_job,
    )
    .await
    .unwrap();

    let media_asset = database
        .media_assets()
        .get(media_asset_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(media_asset.extraction_status.as_deref(), Some("failed"));
    assert!(media_asset.extraction_tmp_path.is_none());
    let retryable_job = database
        .jobs()
        .get_by_logical_key(project_id, Some(media_asset_id), JobType::AudioExtraction)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retryable_job.status, JobStatus::Queued);
    assert!(retryable_job.failure_reason.is_some());

    let final_audio = audio_root.join("audio-assets").join("final.flac");
    fs::create_dir_all(final_audio.parent().unwrap()).unwrap();
    fs::write(&final_audio, b"finalized").unwrap();
    fs::write(audio_tmp_root.join("abandoned.tmp"), b"partial").unwrap();
    let recovery_runtime = WorkerRuntime::new("recovery-test".to_string());
    recovery_runtime
        .recover_abandoned_temp_outputs(&state)
        .await
        .unwrap();
    assert!(!audio_tmp_root.join("abandoned.tmp").exists());
    assert!(final_audio.exists());
}
