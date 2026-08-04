pub fn clear_env() {
    for key in [
        "DATA_STUDIO_DATABASE_URL",
        "DATA_STUDIO_WORKSPACE_DIR",
        "DATA_STUDIO_DATA_DIR",
        "DATA_STUDIO_TEMP_DIR",
        "DATA_STUDIO_AUDIO_DIR",
        "DATA_STUDIO_AUDIO_TEMP_DIR",
        "DATA_STUDIO_STAGING_PAUSE_FREE_BYTES",
        "DATA_STUDIO_STAGING_RESUME_FREE_BYTES",
        "DATA_STUDIO_HOST",
        "DATA_STUDIO_PORT",
    ] {
        std::env::remove_var(key);
    }
}
