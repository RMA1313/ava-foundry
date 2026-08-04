use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
    time,
};

#[derive(Debug, Clone)]
pub struct FfmpegAdapter {
    executable: PathBuf,
}

#[derive(Debug)]
pub struct FfmpegProcess {
    child: tokio::process::Child,
    progress_rx: mpsc::UnboundedReceiver<FfmpegProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegProgress {
    pub raw: String,
    pub out_time_ms: Option<u64>,
    pub progress: Option<String>,
}

#[derive(Debug)]
pub enum FfmpegError {
    MissingExecutable(PathBuf),
}

impl std::fmt::Display for FfmpegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingExecutable(path) => {
                write!(f, "required executable not found: {}", path.display())
            }
        }
    }
}

impl std::error::Error for FfmpegError {}

impl FfmpegAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Result<Self> {
        let executable = executable.into();
        if !executable.exists() {
            return Err(FfmpegError::MissingExecutable(executable).into());
        }
        Ok(Self { executable })
    }

    pub fn build_command(&self, input_path: &Path, output_path: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .arg("-hide_banner")
            .arg("-nostdin")
            .arg("-y")
            .arg("-i")
            .arg(input_path)
            .arg("-vn")
            .arg("-sn")
            .arg("-dn")
            .arg("-c:a")
            .arg("copy")
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .arg(output_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    pub async fn start(&self, input_path: &Path, output_path: &Path) -> Result<FfmpegProcess> {
        let mut command = self.build_command(input_path, output_path);
        let mut child = command.spawn().context("failed to start ffmpeg")?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("ffmpeg missing stdout"))?;
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let progress = parse_progress_line(&line);
                let _ = progress_tx.send(progress);
            }
        });
        Ok(FfmpegProcess { child, progress_rx })
    }

    pub async fn extract(
        &self,
        input_path: &Path,
        output_path: &Path,
        timeout: Duration,
    ) -> Result<Vec<FfmpegProgress>> {
        let mut process = self.start(input_path, output_path).await?;
        let mut progress = Vec::new();
        let deadline = time::Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(time::Instant::now())
                .ok_or_else(|| anyhow!("ffmpeg timed out"))?;
            match time::timeout(remaining, process.next_progress()).await {
                Ok(Some(line)) => progress.push(line),
                Ok(None) => break,
                Err(_) => {
                    process.cancel().await?;
                    return Err(anyhow!("ffmpeg timed out"));
                }
            }
        }
        let output = process.wait().await?;
        if !output.status.success() {
            return Err(anyhow!("ffmpeg failed"));
        }
        if !output_path.exists() {
            return Err(anyhow!("ffmpeg completed without writing output"));
        }
        Ok(progress)
    }
}

impl FfmpegProcess {
    pub async fn next_progress(&mut self) -> Option<FfmpegProgress> {
        self.progress_rx.recv().await
    }

    pub async fn wait(mut self) -> Result<std::process::Output> {
        let status = self.child.wait().await?;
        Ok(std::process::Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    pub async fn cancel(mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }
}

fn parse_progress_line(line: &str) -> FfmpegProgress {
    let out_time_ms = line
        .strip_prefix("out_time_ms=")
        .and_then(|value| value.parse::<u64>().ok());
    let progress = line.strip_prefix("progress=").map(str::to_string);
    FfmpegProgress {
        raw: line.to_string(),
        out_time_ms,
        progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    use tempfile::TempDir;

    fn make_executable(contents: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fake-ffmpeg");
        fs::write(&path, contents).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        (dir, path)
    }

    #[test]
    fn command_arguments_are_deterministic() {
        let (_dir, executable) = make_executable("#!/bin/sh\nexit 0\n");
        let adapter = FfmpegAdapter::new(&executable).unwrap();
        let command =
            adapter.build_command(Path::new("/tmp/input.mp4"), Path::new("/tmp/out.flac"));
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "-hide_banner",
                "-nostdin",
                "-y",
                "-i",
                "/tmp/input.mp4",
                "-vn",
                "-sn",
                "-dn",
                "-c:a",
                "copy",
                "-progress",
                "pipe:1",
                "-nostats",
                "/tmp/out.flac",
            ]
        );
    }

    #[tokio::test]
    async fn fake_executable_reports_progress() {
        let (_dir, executable) = make_executable(
            "#!/bin/sh\nfor output; do :; done\nprintf 'out_time_ms=1000\\nprogress=continue\\n'\nprintf 'out_time_ms=2000\\nprogress=end\\n'\n: > \"$output\"\n",
        );
        let adapter = FfmpegAdapter::new(&executable).unwrap();
        let output = tempfile::NamedTempFile::new().unwrap();
        let progress = adapter
            .extract(
                Path::new("/tmp/input.mp4"),
                output.path(),
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        assert!(progress
            .iter()
            .any(|item| item.progress.as_deref() == Some("continue")));
    }

    #[tokio::test]
    async fn missing_executable_is_rejected() {
        let err = FfmpegAdapter::new(Path::new("/no/such/ffmpeg"));
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn cancellation_terminates_fake_process() {
        let (_dir, executable) = make_executable("#!/bin/sh\nsleep 60\n");
        let adapter = FfmpegAdapter::new(&executable).unwrap();
        let output = tempfile::NamedTempFile::new().unwrap();
        let process = adapter
            .start(Path::new("/tmp/input.mp4"), output.path())
            .await
            .unwrap();
        process.cancel().await.unwrap();
    }
}
