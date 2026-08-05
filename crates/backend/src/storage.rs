use anyhow::{anyhow, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

pub trait Storage {
    fn permanent_path(&self, audio_asset_id: Uuid) -> Result<PathBuf>;
    fn temporary_extraction_path(&self, media_asset_id: Uuid) -> Result<PathBuf>;
    fn temporary_normalization_path(&self, media_asset_id: Uuid) -> Result<PathBuf>;
    fn permanent_fingerprint_path(
        &self,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
    ) -> Result<PathBuf>;
    fn temporary_fingerprint_path(
        &self,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
    ) -> Result<PathBuf>;
    fn ensure_roots(&self) -> Result<()>;
    fn write(&self, path: &Path, bytes: &[u8]) -> Result<()>;
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn read_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>>;
    fn move_file(&self, from: &Path, to: &Path) -> Result<()>;
    fn delete(&self, path: &Path) -> Result<()>;
    fn stat(&self, path: &Path) -> Result<fs::Metadata>;
    fn exists(&self, path: &Path) -> Result<bool>;
}

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

    fn managed_path(root: &Path, segments: &[&str], extension: Option<&str>) -> Result<PathBuf> {
        path_within_root(root, segments, extension)
    }

    fn validate_managed_path<'a>(&self, path: &'a Path) -> Result<&'a Path> {
        if path.starts_with(&self.permanent_root) {
            ensure_root_contained(&self.permanent_root, path)?;
            return Ok(path);
        }
        if path.starts_with(&self.temporary_root) {
            ensure_root_contained(&self.temporary_root, path)?;
            return Ok(path);
        }
        Err(anyhow!("storage path escaped configured root"))
    }

    fn create_parent_dirs(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        Ok(())
    }

    fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("storage path must have a parent: {}", path.display()))?;
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        let tmp_path = parent.join(format!(".{}.tmp", Uuid::new_v4()));
        {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
                .with_context(|| format!("create {}", tmp_path.display()))?;
            file.write_all(bytes)
                .with_context(|| format!("write {}", tmp_path.display()))?;
            file.sync_all()
                .with_context(|| format!("sync {}", tmp_path.display()))?;
        }
        fs::rename(&tmp_path, path)
            .with_context(|| format!("move {} to {}", tmp_path.display(), path.display()))?;
        Ok(())
    }

    pub fn permanent_path(&self, audio_asset_id: Uuid) -> Result<PathBuf> {
        Self::managed_path(
            &self.permanent_root,
            &["audio-assets", &audio_asset_id.to_string()],
            Some("flac"),
        )
    }

    pub fn temporary_extraction_path(&self, media_asset_id: Uuid) -> Result<PathBuf> {
        Self::managed_path(
            &self.temporary_root,
            &["audio-extraction", &media_asset_id.to_string()],
            Some("tmp"),
        )
    }

    pub fn temporary_normalization_path(&self, media_asset_id: Uuid) -> Result<PathBuf> {
        Self::managed_path(
            &self.temporary_root,
            &["audio-normalization", &media_asset_id.to_string()],
            Some("tmp"),
        )
    }

    pub fn permanent_fingerprint_path(
        &self,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
    ) -> Result<PathBuf> {
        Self::managed_path(
            &self.permanent_root,
            &[
                "audio-fingerprints",
                &audio_asset_id.to_string(),
                algorithm,
                version,
            ],
            Some("fp"),
        )
    }

    pub fn temporary_fingerprint_path(
        &self,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
    ) -> Result<PathBuf> {
        Self::managed_path(
            &self.temporary_root,
            &[
                "audio-fingerprints",
                &audio_asset_id.to_string(),
                algorithm,
                version,
            ],
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
        expected_byte_size: i64,
    ) -> Result<PathBuf> {
        let temporary_path = self.validate_managed_path(temporary_path)?;
        let final_path = self.permanent_path(audio_asset_id)?;
        Self::create_parent_dirs(&final_path)?;
        self.finalize_managed_path(temporary_path, &final_path, expected_byte_size)
    }

    pub fn finalize_fingerprint_path(
        &self,
        temporary_path: &Path,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
        expected_byte_size: i64,
    ) -> Result<PathBuf> {
        let temporary_path = self.validate_managed_path(temporary_path)?;
        let final_path = self.permanent_fingerprint_path(audio_asset_id, algorithm, version)?;
        Self::create_parent_dirs(&final_path)?;
        self.finalize_managed_path(temporary_path, &final_path, expected_byte_size)
    }

    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        <Self as Storage>::write(self, path, bytes)
    }

    pub fn read(&self, path: &Path) -> Result<Vec<u8>> {
        <Self as Storage>::read(self, path)
    }

    pub fn read_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
        <Self as Storage>::read_range(self, path, offset, length)
    }

    pub fn move_file(&self, from: &Path, to: &Path) -> Result<()> {
        <Self as Storage>::move_file(self, from, to)
    }

    pub fn delete(&self, path: &Path) -> Result<()> {
        <Self as Storage>::delete(self, path)
    }

    pub fn stat(&self, path: &Path) -> Result<fs::Metadata> {
        <Self as Storage>::stat(self, path)
    }

    pub fn exists(&self, path: &Path) -> Result<bool> {
        <Self as Storage>::exists(self, path)
    }

    fn finalize_managed_path(
        &self,
        temporary_path: &Path,
        final_path: &Path,
        expected_byte_size: i64,
    ) -> Result<PathBuf> {
        if final_path.exists() {
            let final_size = fs::metadata(final_path)
                .with_context(|| format!("metadata {}", final_path.display()))?
                .len() as i64;
            if final_size != expected_byte_size {
                return Err(anyhow!(
                    "existing finalized file has unexpected size: {}",
                    final_path.display()
                ));
            }
            if temporary_path.exists() {
                let temporary_size = fs::metadata(temporary_path)
                    .with_context(|| format!("metadata {}", temporary_path.display()))?
                    .len() as i64;
                if temporary_size != expected_byte_size {
                    return Err(anyhow!(
                        "temporary file conflicts with finalized file: {}",
                        temporary_path.display()
                    ));
                }
                return Err(anyhow!(
                    "temporary file still exists after finalization: {}",
                    temporary_path.display()
                ));
            }
            return Ok(final_path.to_path_buf());
        }
        if !temporary_path.exists() {
            return Err(anyhow!(
                "temporary file missing before finalization: {}",
                temporary_path.display()
            ));
        }
        let temporary_size = fs::metadata(temporary_path)
            .with_context(|| format!("metadata {}", temporary_path.display()))?
            .len() as i64;
        if temporary_size != expected_byte_size {
            return Err(anyhow!(
                "temporary file size does not match validation: {}",
                temporary_path.display()
            ));
        }
        fs::rename(temporary_path, final_path).with_context(|| {
            format!(
                "move {} to {}",
                temporary_path.display(),
                final_path.display()
            )
        })?;
        let final_size = fs::metadata(final_path)
            .with_context(|| format!("metadata {}", final_path.display()))?
            .len() as i64;
        if final_size != expected_byte_size {
            return Err(anyhow!(
                "finalized file has unexpected size: {}",
                final_path.display()
            ));
        }
        Ok(final_path.to_path_buf())
    }
}

impl Storage for AudioStorageConfig {
    fn permanent_path(&self, audio_asset_id: Uuid) -> Result<PathBuf> {
        Self::permanent_path(self, audio_asset_id)
    }

    fn temporary_extraction_path(&self, media_asset_id: Uuid) -> Result<PathBuf> {
        Self::temporary_extraction_path(self, media_asset_id)
    }

    fn temporary_normalization_path(&self, media_asset_id: Uuid) -> Result<PathBuf> {
        Self::temporary_normalization_path(self, media_asset_id)
    }

    fn permanent_fingerprint_path(
        &self,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
    ) -> Result<PathBuf> {
        Self::permanent_fingerprint_path(self, audio_asset_id, algorithm, version)
    }

    fn temporary_fingerprint_path(
        &self,
        audio_asset_id: Uuid,
        algorithm: &str,
        version: &str,
    ) -> Result<PathBuf> {
        Self::temporary_fingerprint_path(self, audio_asset_id, algorithm, version)
    }

    fn ensure_roots(&self) -> Result<()> {
        Self::ensure_roots(self)
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let path = self.validate_managed_path(path)?;
        Self::atomic_write(path, bytes)
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let path = self.validate_managed_path(path)?;
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .with_context(|| format!("read {}", path.display()))?;
        Ok(bytes)
    }

    fn read_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
        let path = self.validate_managed_path(path)?;
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        file.seek(SeekFrom::Start(offset))
            .with_context(|| format!("seek {}", path.display()))?;
        let mut limited = file.take(length);
        let mut bytes = Vec::new();
        limited
            .read_to_end(&mut bytes)
            .with_context(|| format!("read {}", path.display()))?;
        Ok(bytes)
    }

    fn move_file(&self, from: &Path, to: &Path) -> Result<()> {
        let from = self.validate_managed_path(from)?;
        let to = self.validate_managed_path(to)?;
        Self::create_parent_dirs(to)?;
        fs::rename(from, to).with_context(|| format!("move {} to {}", from.display(), to.display()))
    }

    fn delete(&self, path: &Path) -> Result<()> {
        let path = self.validate_managed_path(path)?;
        match fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("delete {}", path.display())),
        }
    }

    fn stat(&self, path: &Path) -> Result<fs::Metadata> {
        let path = self.validate_managed_path(path)?;
        fs::metadata(path).with_context(|| format!("metadata {}", path.display()))
    }

    fn exists(&self, path: &Path) -> Result<bool> {
        let path = self.validate_managed_path(path)?;
        Ok(path.exists())
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
    ensure_root_contained(root, &path)?;
    Ok(path)
}

fn ensure_root_contained(root: &Path, path: &Path) -> Result<()> {
    if !root.is_absolute() || !path.is_absolute() || !path.starts_with(root) {
        return Err(anyhow!("storage path escaped configured root"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| anyhow!("storage path escaped configured root"))?;
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(anyhow!("storage path escaped configured root"));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::AudioStorageConfig;
    use std::fs;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn fingerprint_paths_are_deterministic_and_contained() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = AudioStorageConfig::new(
            temp_dir.path().join("audio"),
            temp_dir.path().join("audio-tmp"),
        );
        let audio_asset_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let path = config
            .permanent_fingerprint_path(audio_asset_id, "chromaprint", "1")
            .expect("path");
        assert_eq!(
            path,
            temp_dir
                .path()
                .join("audio")
                .join("audio-fingerprints")
                .join(audio_asset_id.to_string())
                .join("chromaprint")
                .join("1.fp")
        );
        let temp_path = config
            .temporary_fingerprint_path(audio_asset_id, "chromaprint", "1")
            .expect("temp path");
        assert!(temp_path.starts_with(temp_dir.path().join("audio-tmp")));
    }

    #[test]
    fn path_traversal_is_rejected_for_managed_paths() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = AudioStorageConfig::new(
            temp_dir.path().join("audio"),
            temp_dir.path().join("audio-tmp"),
        );
        let malicious = temp_dir.path().join("audio").join("..").join("evil.txt");
        let err = config
            .write(&malicious, b"evil")
            .expect_err("traversal should be rejected");
        assert!(err.to_string().contains("escaped configured root"));
    }

    #[test]
    fn read_range_and_delete_are_supported() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = AudioStorageConfig::new(
            temp_dir.path().join("audio"),
            temp_dir.path().join("audio-tmp"),
        );
        let path = config.permanent_path(Uuid::new_v4()).expect("path");
        config.write(&path, b"abcdef").expect("write");

        assert_eq!(config.read(&path).expect("read"), b"abcdef");
        assert_eq!(config.read_range(&path, 2, 3).expect("range"), b"cde");
        assert!(config.exists(&path).expect("exists"));
        assert_eq!(config.stat(&path).expect("stat").len(), 6);

        config.delete(&path).expect("delete");
        config.delete(&path).expect("delete again");
        assert!(!config.exists(&path).expect("exists after delete"));
    }

    #[test]
    fn finalize_fingerprint_path_renames_atomically_into_place() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = AudioStorageConfig::new(
            temp_dir.path().join("audio"),
            temp_dir.path().join("audio-tmp"),
        );
        let audio_asset_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let temporary_path = config
            .temporary_fingerprint_path(audio_asset_id, "chromaprint", "1")
            .expect("temp path");
        if let Some(parent) = temporary_path.parent() {
            fs::create_dir_all(parent).expect("temp dir");
        }
        fs::write(&temporary_path, b"fingerprint").expect("temp file");

        let final_path = config
            .finalize_fingerprint_path(&temporary_path, audio_asset_id, "chromaprint", "1", 11)
            .expect("finalize");

        assert_eq!(
            final_path,
            config
                .permanent_fingerprint_path(audio_asset_id, "chromaprint", "1")
                .expect("permanent path")
        );
        assert!(final_path.exists());
        assert!(!temporary_path.exists());
    }
}
