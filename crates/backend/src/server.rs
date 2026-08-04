use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use crate::{db::Database, http, worker::StagingCapacityGuard};

#[derive(Clone)]
pub struct AppState {
    pub database: Database,
    pub staging_dir: PathBuf,
    pub staging_capacity_guard: Arc<StagingCapacityGuard>,
}

pub async fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    let app = http::router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
