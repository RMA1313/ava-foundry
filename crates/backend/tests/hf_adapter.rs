mod support;

use backend::hf_adapter;
use std::path::Path;

#[tokio::test]
async fn local_fixture_dataset_is_enumerated_and_streamed() {
    support::clear_env();
    let dataset_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("services")
        .join("inference-worker")
        .join("tests")
        .join("fixtures")
        .join("hf_dataset");
    let splits = hf_adapter::list_splits(&dataset_dir)
        .await
        .expect("splits should load");
    assert_eq!(splits, vec!["train".to_string(), "validation".to_string()]);

    let mut records = Vec::new();
    hf_adapter::stream_records(&dataset_dir, "train", "audio", |record| {
        records.push(record);
        Ok(())
    })
    .await
    .expect("records should stream");

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].audio_path, "audio/train-1.wav");
    assert_eq!(records[0].metadata["speaker"], "alpha");
}
