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
pub struct DownloadCommandAdapter {
    executable: PathBuf,
}

#[derive(Debug)]
pub struct DownloadProcess {
    child: tokio::process::Child,
    progress_rx: mpsc::UnboundedReceiver<String>,
}

impl DownloadCommandAdapter {
    pub fn new(executable: impl Into<PathBuf>) -> Result<Self> {
        let executable = executable.into();
        if !executable.exists() {
            return Err(anyhow!(
                "required executable not found: {}",
                executable.display()
            ));
        }
        Ok(Self { executable })
    }

    pub fn build_command(&self, url: &str, output_path: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .arg("--newline")
            .arg("--no-playlist")
            .arg("--output")
            .arg(output_path)
            .arg(url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    pub async fn start(&self, url: &str, output_path: &Path) -> Result<DownloadProcess> {
        let mut command = self.build_command(url, output_path);
        let mut child = command
            .spawn()
            .context("failed to start download process")?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("download process missing stdout"))?;
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = progress_tx.send(line);
            }
        });
        Ok(DownloadProcess { child, progress_rx })
    }
}

impl DownloadProcess {
    pub async fn next_progress(&mut self) -> Option<String> {
        self.progress_rx.recv().await
    }

    pub async fn wait(mut self) -> Result<std::process::ExitStatus> {
        Ok(self.child.wait().await?)
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
    use tokio::time::{timeout, Duration};

    #[test]
    fn command_arguments_are_deterministic() {
        let dir = TempDir::new().unwrap();
        let executable = dir.path().join("fake");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$TMPDIR/args.txt\"\n",
        )
        .unwrap();
        let mut perms = fs::metadata(&executable).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&executable, perms).unwrap();
        let adapter = DownloadCommandAdapter::new(&executable).unwrap();
        let output_dir = dir.path().join("tmp");
        fs::create_dir_all(&output_dir).unwrap();
        std::env::set_var("TMPDIR", &output_dir);
        let process = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(adapter.start("https://example.com/video", Path::new("/tmp/out.bin")))
            .unwrap();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(process.wait())
            .unwrap();
        let args = fs::read_to_string(output_dir.join("args.txt")).unwrap();
        assert_eq!(
            args,
            "--newline\n--no-playlist\n--output\n/tmp/out.bin\nhttps://example.com/video\n"
        );
    }

    #[tokio::test]
    async fn fake_process_can_be_cancelled() {
        let dir = TempDir::new().unwrap();
        let executable = dir.path().join("fake");
        fs::write(&executable, "#!/bin/sh\nsleep 60\n").unwrap();
        let mut perms = fs::metadata(&executable).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&executable, perms).unwrap();
        let adapter = DownloadCommandAdapter::new(&executable).unwrap();
        let process = adapter
            .start("https://example.com/video", Path::new("/tmp/out.bin"))
            .await
            .unwrap();
        timeout(Duration::from_millis(50), process.cancel())
            .await
            .unwrap()
            .unwrap();
    }
}
