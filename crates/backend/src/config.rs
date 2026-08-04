use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database_url: String,
    pub workspace_dir: PathBuf,
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn load_from_env() -> Result<Self> {
        let database_url = std::env::var("DATA_STUDIO_DATABASE_URL")
            .context("missing required environment variable DATA_STUDIO_DATABASE_URL")?;
        let workspace_dir = PathBuf::from(std::env::var("DATA_STUDIO_WORKSPACE_DIR").unwrap_or_else(|_| ".".to_string()));
        let data_dir = PathBuf::from(std::env::var("DATA_STUDIO_DATA_DIR").unwrap_or_else(|_| workspace_dir.join("data").to_string_lossy().into_owned()));
        let temp_dir = PathBuf::from(std::env::var("DATA_STUDIO_TEMP_DIR").unwrap_or_else(|_| data_dir.join("tmp").to_string_lossy().into_owned()));
        let host = std::env::var("DATA_STUDIO_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("DATA_STUDIO_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .context("DATA_STUDIO_PORT must be a valid u16")?;

        Ok(Self {
            database_url,
            workspace_dir,
            data_dir,
            temp_dir,
            host,
            port,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_database_url_fails() {
        std::env::remove_var("DATA_STUDIO_DATABASE_URL");
        let err = AppConfig::load_from_env().unwrap_err();
        assert!(err.to_string().contains("DATA_STUDIO_DATABASE_URL"));
    }
}

