use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::{process::Command, time};

#[derive(Debug, Clone)]
pub struct FfprobeAdapter {
    executable: PathBuf,
}

#[derive(Debug)]
pub struct FfprobeProcess {
    child: tokio::process::Child,
    stdout_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FfprobeOutput {
    pub format: Option<FfprobeFormat>,
    #[serde(default)]
    pub streams: Vec<FfprobeStream>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FfprobeFormat {
    pub format_name: Option<String>,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FfprobeStream {
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub sample_rate: Option<String>,
    pub channels: Option<i32>,
}

#[derive(Debug)]
pub enum FfprobeError {
    MissingExecutable(PathBuf),
}

impl std::fmt::Display for FfprobeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingExecutable(path) => {
                write!(f, "required executable not found: {}", path.display())
            }
        }
    }
}

impl std::error::Error for FfprobeError {}

impl FfprobeAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Result<Self> {
        let executable = executable.into();
        if !executable.exists() {
            return Err(FfprobeError::MissingExecutable(executable).into());
        }
        Ok(Self { executable })
    }

    pub fn build_command(&self, input_path: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .arg("-v")
            .arg("error")
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg("-show_streams")
            .arg(input_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    pub async fn start(&self, input_path: &Path) -> Result<FfprobeProcess> {
        let mut command = self.build_command(input_path);
        let mut child = command.spawn().context("failed to start ffprobe")?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("ffprobe missing stdout"))?;
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let mut stdout = stdout;
            if stdout.read_to_end(&mut buf).await.is_ok() {
                let _ = stdout_tx.send(buf);
            }
        });
        Ok(FfprobeProcess { child, stdout_rx })
    }

    pub async fn probe(&self, input_path: &Path, timeout: Duration) -> Result<FfprobeOutput> {
        let process = self.start(input_path).await?;
        let output = time::timeout(timeout, process.wait())
            .await
            .map_err(|_| anyhow!("ffprobe timed out"))??;
        if !output.status.success() {
            return Err(anyhow!("ffprobe failed"));
        }
        let parsed: FfprobeOutput =
            serde_json::from_slice(&output.stdout).context("failed to parse ffprobe json")?;
        Ok(parsed)
    }
}

impl FfprobeProcess {
    pub async fn wait(mut self) -> Result<std::process::Output> {
        let stdout = self.stdout_rx.recv().await.unwrap_or_default();
        let status = self.child.wait().await?;
        Ok(std::process::Output {
            status,
            stdout,
            stderr: Vec::new(),
        })
    }

    pub async fn cancel(mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    use tempfile::TempDir;

    fn make_executable(contents: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fake-ffprobe");
        fs::write(&path, contents).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        (dir, path)
    }

    #[tokio::test]
    async fn probe_parses_json_from_fake_executable() {
        let (_dir, executable) = make_executable(
            "#!/bin/sh\ncat <<'JSON'\n{\"format\":{\"format_name\":\"matroska,webm\",\"duration\":\"12.5\"},\"streams\":[{\"codec_type\":\"audio\",\"codec_name\":\"opus\",\"sample_rate\":\"48000\",\"channels\":2}]}\nJSON\n",
        );
        let adapter = FfprobeAdapter::new(&executable).unwrap();
        let output = adapter
            .probe(Path::new("/tmp/input.mkv"), Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(
            output.format.unwrap().format_name.as_deref(),
            Some("matroska,webm")
        );
        assert_eq!(output.streams[0].codec_name.as_deref(), Some("opus"));
    }

    #[tokio::test]
    async fn missing_executable_is_rejected() {
        let err = FfprobeAdapter::new(Path::new("/no/such/ffprobe"));
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn malformed_json_fails_deterministically() {
        let (_dir, executable) = make_executable("#!/bin/sh\necho not-json\n");
        let adapter = FfprobeAdapter::new(&executable).unwrap();
        let err = adapter
            .probe(Path::new("/tmp/input.mkv"), Duration::from_secs(1))
            .await;
        let message = err.unwrap_err().to_string();
        assert!(
            message.contains("failed to parse ffprobe json") || message.contains("expected value")
        );
    }

    #[tokio::test]
    async fn cancellation_terminates_fake_process() {
        let (_dir, executable) = make_executable("#!/bin/sh\nsleep 60\n");
        let adapter = FfprobeAdapter::new(&executable).unwrap();
        time::sleep(Duration::from_millis(10)).await;
        let process = adapter.start(Path::new("/tmp/input.mkv")).await.unwrap();
        process.cancel().await.unwrap();
    }
}
