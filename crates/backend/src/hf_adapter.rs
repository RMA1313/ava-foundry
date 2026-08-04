use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, process::Stdio};
use tokio::{io::AsyncBufReadExt, process::Command};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HuggingFaceRecord {
    pub audio_path: String,
    pub metadata: Value,
}

pub async fn list_splits(dataset_dir: &Path) -> Result<Vec<String>> {
    let pythonpath = pythonpath();
    let output = Command::new("python3")
        .arg("-m")
        .arg("inference_worker.hf_adapter")
        .arg("list-splits")
        .arg("--dataset-dir")
        .arg(dataset_dir)
        .env("PYTHONPATH", pythonpath)
        .output()
        .await
        .context("failed to run hugging face adapter")?;
    if !output.status.success() {
        return Err(anyhow!(
            "hugging face adapter failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub async fn stream_records<F>(
    dataset_dir: &Path,
    split: &str,
    audio_column: &str,
    mut on_record: F,
) -> Result<()>
where
    F: FnMut(HuggingFaceRecord) -> Result<()>,
{
    let pythonpath = pythonpath();
    let mut child = Command::new("python3")
        .arg("-m")
        .arg("inference_worker.hf_adapter")
        .arg("stream-records")
        .arg("--dataset-dir")
        .arg(dataset_dir)
        .arg("--split")
        .arg(split)
        .arg("--audio-column")
        .arg(audio_column)
        .env("PYTHONPATH", pythonpath)
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to run hugging face adapter")?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing stdout"))?;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        let record: HuggingFaceRecord = serde_json::from_str(&line)?;
        on_record(record)?;
    }
    let status = child.wait().await?;
    if !status.success() {
        return Err(anyhow!("hugging face adapter exited with {status}"));
    }
    Ok(())
}

fn pythonpath() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("services")
        .join("inference-worker")
        .join("src");
    root.to_string_lossy().to_string()
}
