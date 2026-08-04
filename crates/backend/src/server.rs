use std::net::SocketAddr;

use anyhow::Result;
use axum::Router;

use crate::{db::Database, http};

#[derive(Clone)]
pub struct AppState {
    pub database: Database,
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
