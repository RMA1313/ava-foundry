mod support;

use backend::{
    db::Database,
    ingestion,
    job::{JobStatus, JobType},
    migrations,
    server::AppState,
    source::SourceType,
    worker::{media_ingestion_handler, WorkerRuntime},
};
use chrono::Utc;
use sqlx::PgPool;
use std::{env, fs, path::Path};
use tempfile::TempDir;
use tokio::sync::oneshot;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
        });
    let schema_name = format!("ingestion_worker_test_{}", Uuid::new_v4().simple());
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

fn fake_downloader(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("fake-yt-dlp");
    fs::write(
        &path,
        r#"#!/bin/sh
out="$4"
echo "10%"
echo "50%"
echo "100%"
printf "downloaded" > "$out"
"#,
    )
    .expect("script should write");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("permissions");
    }
    path
}

#[tokio::test]
async fn worker_runtime_processes_media_ingestion_and_completes_job() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
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
        .expect("source should be created");
    let job = database
        .jobs()
        .create(
            project_id,
            Some(source.id),
            JobType::MediaIngestion,
            0,
            Utc::now(),
        )
        .await
        .expect("job should be created");
    let temp_dir = TempDir::new().expect("temp dir");
    let executable = fake_downloader(&temp_dir);
    let database_for_runtime = database.clone();
    let runtime = WorkerRuntime::new("worker-test".to_string())
        .with_handler(
            JobType::MediaIngestion,
            media_ingestion_handler(
                temp_dir.path(),
                backend::download::DownloadCommandAdapter::new(executable).unwrap(),
            ),
        )
        .with_poll_interval(std::time::Duration::from_millis(5));
    let state = AppState { database };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let run = tokio::spawn(async move {
        runtime
            .run(state, async move {
                let _ = shutdown_rx.await;
            })
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let _ = shutdown_tx.send(());
    run.await
        .expect("task should join")
        .expect("runtime should succeed");

    let fetched = database_for_runtime
        .jobs()
        .get(job.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.status, JobStatus::Completed);
    let staged = temp_dir.path().join(format!("{}.bin", source.id));
    assert!(Path::new(&staged).exists());
}

#[tokio::test]
async fn local_file_import_reuses_completed_copy_and_ignores_partial_temp_files() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = seed_project(&pool).await;
    let database = Database::from_pool(pool.clone());
    let temp_dir = TempDir::new().expect("temp dir");
    let source_path = temp_dir.path().join("sample.wav");
    fs::write(&source_path, b"audio").expect("source");
    let staging_dir = temp_dir.path().join("staging");
    fs::create_dir_all(&staging_dir).expect("staging");
    fs::write(staging_dir.join("partial.part"), b"partial").expect("tmp");

    let result = ingestion::register_local_file(&database, project_id, &source_path)
        .await
        .expect("registration should work");
    let (staged_path, bytes) = ingestion::import_local_file_to_staging(
        &database,
        result.source_id,
        &source_path,
        &staging_dir,
    )
    .await
    .expect("import should work");
    assert_eq!(bytes, 5);
    assert!(staged_path.exists());
    assert_eq!(fs::read(&staged_path).expect("staged"), b"audio");

    let repeated = ingestion::import_local_file_to_staging(
        &database,
        result.source_id,
        &source_path,
        &staging_dir,
    )
    .await
    .expect("reimport should reuse existing file");
    assert_eq!(repeated.0, staged_path);
}
