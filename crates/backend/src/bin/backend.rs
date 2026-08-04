use std::net::SocketAddr;

use anyhow::Result;
use backend::{config::AppConfig, db::Database, server::{self, AppState}};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load_from_env()?;
    let database = Database::connect(&config.database_url).await?;
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    server::serve(addr, AppState { database }).await
}

