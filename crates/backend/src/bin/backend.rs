use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use backend::{
    config::AppConfig,
    db::Database,
    server::{self, AppState},
    storage::AudioStorageConfig,
    worker::{DefaultFilesystemMetrics, StagingCapacityGuard},
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load_from_env()?;
    let database = Database::connect(&config.database_url).await?;
    let recovered = database
        .jobs()
        .recover_expired_leases(chrono::Utc::now())
        .await?;
    info!(
        recovered = recovered.len(),
        "recovered expired leases during startup"
    );
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let staging_capacity_guard = Arc::new(StagingCapacityGuard::new(
        config.staging_pause_free_bytes,
        config.staging_resume_free_bytes,
        DefaultFilesystemMetrics,
    ));
    let audio_storage = AudioStorageConfig::new(config.audio_dir, config.audio_temp_dir);
    server::serve(
        addr,
        AppState {
            database,
            staging_dir: config.temp_dir,
            audio_storage,
            audio_normalization_profile: config.audio_normalization_profile,
            staging_capacity_guard,
        },
    )
    .await
}
