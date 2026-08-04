use anyhow::Result;
use backend::{config::AppConfig, migrations};

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load_from_env()?;
    migrations::run(&config.database_url).await
}
