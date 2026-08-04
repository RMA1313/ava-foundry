use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStorageConfig {
    pub permanent_root: PathBuf,
    pub temporary_root: PathBuf,
}

impl AudioStorageConfig {
    pub fn new(permanent_root: PathBuf, temporary_root: PathBuf) -> Self {
        Self {
            permanent_root,
            temporary_root,
        }
    }

    pub fn permanent_path(&self, audio_asset_id: Uuid) -> Result<PathBuf> {
        path_within_root(
            &self.permanent_root,
            &["audio-assets", &audio_asset_id.to_string()],
            Some("flac"),
        )
    }

    pub fn temporary_extraction_path(&self, media_asset_id: Uuid) -> Result<PathBuf> {
        path_within_root(
            &self.temporary_root,
            &["audio-extraction", &media_asset_id.to_string()],
            Some("tmp"),
        )
    }

    pub fn ensure_roots(&self) -> Result<()> {
        fs::create_dir_all(&self.permanent_root)
            .with_context(|| format!("create {}", self.permanent_root.display()))?;
        fs::create_dir_all(&self.temporary_root)
            .with_context(|| format!("create {}", self.temporary_root.display()))?;
        Ok(())
    }

    pub fn finalize_audio_path(
        &self,
        temporary_path: &Path,
        audio_asset_id: Uuid,
    ) -> Result<PathBuf> {
        let final_path = self.permanent_path(audio_asset_id)?;
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::rename(temporary_path, &final_path).with_context(|| {
            format!(
                "move {} to {}",
                temporary_path.display(),
                final_path.display()
            )
        })?;
        Ok(final_path)
    }
}

fn path_within_root(root: &Path, segments: &[&str], extension: Option<&str>) -> Result<PathBuf> {
    if !root.is_absolute() {
        return Err(anyhow!("storage root must be absolute: {}", root.display()));
    }
    let mut path = root.to_path_buf();
    for segment in segments {
        validate_segment(segment)?;
        path.push(segment);
    }
    if let Some(extension) = extension {
        validate_segment(extension)?;
        path.set_extension(extension);
    }
    if !path.starts_with(root) {
        return Err(anyhow!("storage path escaped configured root"));
    }
    Ok(path)
}

fn validate_segment(segment: &str) -> Result<()> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(anyhow!("invalid storage path segment"));
    }
    if Path::new(segment).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(anyhow!("invalid storage path segment"));
    }
    Ok(())
}
