use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{anyhow, Context, Result};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
};

#[derive(Debug, Clone)]
pub struct FingerprintAdapter {
    executable: PathBuf,
}

#[derive(Debug)]
pub struct FingerprintProcess {
    child: tokio::process::Child,
    stdout_rx: mpsc::UnboundedReceiver<String>,
}

#[derive(Debug)]
pub enum FingerprintError {
    MissingExecutable(PathBuf),
}

impl std::fmt::Display for FingerprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingExecutable(path) => {
                write!(f, "required executable not found: {}", path.display())
            }
        }
    }
}

impl std::error::Error for FingerprintError {}

impl FingerprintAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Result<Self> {
        let executable = executable.into();
        if !executable.exists() {
            return Err(FingerprintError::MissingExecutable(executable).into());
        }
        Ok(Self { executable })
    }

    pub fn build_command(&self, input_path: &Path, output_path: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .arg(input_path)
            .arg(output_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    pub async fn start(&self, input_path: &Path, output_path: &Path) -> Result<FingerprintProcess> {
        let mut command = self.build_command(input_path, output_path);
        let mut child = command
            .spawn()
            .context("failed to start fingerprint executable")?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("fingerprint executable missing stdout"))?;
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stdout_tx.send(line);
            }
        });
        Ok(FingerprintProcess { child, stdout_rx })
    }
}

impl FingerprintProcess {
    pub async fn next_stdout(&mut self) -> Option<String> {
        self.stdout_rx.recv().await
    }

    pub async fn wait(mut self) -> Result<std::process::Output> {
        let status = self.child.wait().await?;
        Ok(std::process::Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}
