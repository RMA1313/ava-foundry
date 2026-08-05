mod support;

use backend::{
    migrations,
    stage_run::{StageRun, StageRunRepository},
};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

async fn test_pool() -> PgPool {
    let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
        .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://postgres:postgres@localhost:5432/data_studio".to_string()
        });
    let schema_name = format!("stage_run_repo_test_{}", Uuid::new_v4().simple());
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
async fn stage_runs_can_be_recorded_and_listed() {
    support::clear_env();
    let pool = test_pool().await;
    let project_id = Uuid::new_v4();
    sqlx::query("insert into projects (id, name, description, created_at, updated_at) values ($1, $2, null, now(), now())")
        .bind(project_id)
        .bind("Research")
        .execute(&pool)
        .await
        .unwrap();
    let repo = StageRunRepository::new(pool.clone());
    let run = repo
        .insert(
            &pool,
            StageRun {
                id: Uuid::new_v4(),
                project_id,
                media_asset_id: None,
                audio_asset_id: None,
                job_id: None,
                stage_name: "audio_validation".to_string(),
                status: "completed".to_string(),
                queue_wait_ms: 15,
                wall_time_ms: 25,
                input_bytes: 100,
                output_bytes: 200,
                input_duration_seconds: 12.0,
                throughput: Some(0.48),
                failure_reason: None,
                cpu_percent: None,
                memory_bytes: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();
    assert_eq!(run.stage_name, "audio_validation");
    let listed = repo.list_by_project(project_id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].queue_wait_ms, 15);
}
