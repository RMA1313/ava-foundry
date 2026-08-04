mod support;

use backend::storage::AudioStorageConfig;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn path_generation_is_deterministic_and_contained() {
    support::clear_env();
    let temp = tempdir().expect("tempdir");
    let config = AudioStorageConfig::new(temp.path().join("audio"), temp.path().join("tmp"));
    let audio_asset_id = Uuid::new_v4();
    let media_asset_id = Uuid::new_v4();

    let permanent_a = config.permanent_path(audio_asset_id).expect("path");
    let permanent_b = config.permanent_path(audio_asset_id).expect("path");
    let temporary_a = config
        .temporary_extraction_path(media_asset_id)
        .expect("path");
    let temporary_b = config
        .temporary_extraction_path(media_asset_id)
        .expect("path");

    assert_eq!(permanent_a, permanent_b);
    assert_eq!(temporary_a, temporary_b);
    assert!(permanent_a.starts_with(temp.path().join("audio")));
    assert!(temporary_a.starts_with(temp.path().join("tmp")));
}

#[test]
fn missing_directories_are_created_and_finalization_moves_atomically() {
    support::clear_env();
    let temp = tempdir().expect("tempdir");
    let config = AudioStorageConfig::new(temp.path().join("audio"), temp.path().join("tmp"));
    let media_asset_id = Uuid::new_v4();
    let audio_asset_id = Uuid::new_v4();

    config.ensure_roots().expect("roots should be created");
    let temporary_path = config
        .temporary_extraction_path(media_asset_id)
        .expect("temp path");
    if let Some(parent) = temporary_path.parent() {
        fs::create_dir_all(parent).expect("temp parent");
    }
    fs::write(&temporary_path, b"audio").expect("temporary audio");

    let final_path = config
        .finalize_audio_path(&temporary_path, audio_asset_id)
        .expect("finalization");
    assert!(final_path.exists());
    assert!(!temporary_path.exists());
    assert_eq!(
        fs::read(&final_path).expect("final content"),
        b"audio".to_vec()
    );
    assert!(Path::new(&final_path).starts_with(temp.path().join("audio")));
}
