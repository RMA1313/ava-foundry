use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database_url: String,
    pub workspace_dir: PathBuf,
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub staging_pause_free_bytes: u64,
    pub staging_resume_free_bytes: u64,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn load_from_env() -> Result<Self> {
        let database_url = std::env::var("DATA_STUDIO_DATABASE_URL")
            .context("missing required environment variable DATA_STUDIO_DATABASE_URL")?;
        let workspace_dir = PathBuf::from(
            std::env::var("DATA_STUDIO_WORKSPACE_DIR").unwrap_or_else(|_| ".".to_string()),
        );
        let data_dir = PathBuf::from(
            std::env::var("DATA_STUDIO_DATA_DIR")
                .unwrap_or_else(|_| workspace_dir.join("data").to_string_lossy().into_owned()),
        );
        let temp_dir = PathBuf::from(
            std::env::var("DATA_STUDIO_TEMP_DIR")
                .unwrap_or_else(|_| data_dir.join("tmp").to_string_lossy().into_owned()),
        );
        let staging_pause_free_bytes = std::env::var("DATA_STUDIO_STAGING_PAUSE_FREE_BYTES")
            .unwrap_or_else(|_| "1073741824".to_string())
            .parse::<u64>()
            .context("DATA_STUDIO_STAGING_PAUSE_FREE_BYTES must be a valid u64")?;
        let staging_resume_free_bytes = std::env::var("DATA_STUDIO_STAGING_RESUME_FREE_BYTES")
            .unwrap_or_else(|_| "2147483648".to_string())
            .parse::<u64>()
            .context("DATA_STUDIO_STAGING_RESUME_FREE_BYTES must be a valid u64")?;
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
            staging_pause_free_bytes,
            staging_resume_free_bytes,
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
