use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioNormalizationProfile {
    pub format: String,
    pub codec: String,
    pub sample_rate_hz: i32,
    pub channels: i32,
}

impl AudioNormalizationProfile {
    pub fn default_flac_16khz_mono() -> Self {
        Self {
            format: "flac".to_string(),
            codec: "flac".to_string(),
            sample_rate_hz: 16_000,
            channels: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database_url: String,
    pub workspace_dir: PathBuf,
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub audio_dir: PathBuf,
    pub audio_temp_dir: PathBuf,
    pub staging_pause_free_bytes: u64,
    pub staging_resume_free_bytes: u64,
    pub audio_normalization_profile: AudioNormalizationProfile,
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
        let audio_dir = PathBuf::from(
            std::env::var("DATA_STUDIO_AUDIO_DIR")
                .unwrap_or_else(|_| data_dir.join("audio").to_string_lossy().into_owned()),
        );
        let audio_temp_dir = PathBuf::from(
            std::env::var("DATA_STUDIO_AUDIO_TEMP_DIR")
                .unwrap_or_else(|_| temp_dir.join("audio").to_string_lossy().into_owned()),
        );
        let staging_pause_free_bytes = std::env::var("DATA_STUDIO_STAGING_PAUSE_FREE_BYTES")
            .unwrap_or_else(|_| "1073741824".to_string())
            .parse::<u64>()
            .context("DATA_STUDIO_STAGING_PAUSE_FREE_BYTES must be a valid u64")?;
        let staging_resume_free_bytes = std::env::var("DATA_STUDIO_STAGING_RESUME_FREE_BYTES")
            .unwrap_or_else(|_| "2147483648".to_string())
            .parse::<u64>()
            .context("DATA_STUDIO_STAGING_RESUME_FREE_BYTES must be a valid u64")?;
        let audio_normalization_profile = AudioNormalizationProfile {
            format: std::env::var("DATA_STUDIO_AUDIO_NORMALIZATION_FORMAT")
                .unwrap_or_else(|_| "flac".to_string()),
            codec: std::env::var("DATA_STUDIO_AUDIO_NORMALIZATION_CODEC")
                .unwrap_or_else(|_| "flac".to_string()),
            sample_rate_hz: std::env::var("DATA_STUDIO_AUDIO_NORMALIZATION_SAMPLE_RATE_HZ")
                .unwrap_or_else(|_| "16000".to_string())
                .parse::<i32>()
                .context("DATA_STUDIO_AUDIO_NORMALIZATION_SAMPLE_RATE_HZ must be a valid i32")?,
            channels: std::env::var("DATA_STUDIO_AUDIO_NORMALIZATION_CHANNELS")
                .unwrap_or_else(|_| "1".to_string())
                .parse::<i32>()
                .context("DATA_STUDIO_AUDIO_NORMALIZATION_CHANNELS must be a valid i32")?,
        };
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
            audio_dir,
            audio_temp_dir,
            staging_pause_free_bytes,
            staging_resume_free_bytes,
            audio_normalization_profile,
            host,
            port,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn missing_database_url_fails() {
        let _guard = env_lock().lock().unwrap();
        std::env::remove_var("DATA_STUDIO_DATABASE_URL");
        let err = AppConfig::load_from_env().unwrap_err();
        assert!(err.to_string().contains("DATA_STUDIO_DATABASE_URL"));
    }

    #[test]
    fn default_audio_profile_is_flac_16khz_mono() {
        let _guard = env_lock().lock().unwrap();
        std::env::set_var("DATA_STUDIO_DATABASE_URL", "postgres://localhost/test");
        std::env::set_var("DATA_STUDIO_WORKSPACE_DIR", ".");
        std::env::remove_var("DATA_STUDIO_AUDIO_NORMALIZATION_FORMAT");
        std::env::remove_var("DATA_STUDIO_AUDIO_NORMALIZATION_CODEC");
        std::env::remove_var("DATA_STUDIO_AUDIO_NORMALIZATION_SAMPLE_RATE_HZ");
        std::env::remove_var("DATA_STUDIO_AUDIO_NORMALIZATION_CHANNELS");
        let config = AppConfig::load_from_env().unwrap();
        assert_eq!(
            config.audio_normalization_profile,
            AudioNormalizationProfile::default_flac_16khz_mono()
        );
    }
}
