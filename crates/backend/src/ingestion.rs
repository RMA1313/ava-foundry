use std::{
    fs,
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use walkdir::WalkDir;

use crate::{db::Database, job::JobType, source::SourceType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationResult {
    pub source_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvImportSummary {
    pub imported: u32,
    pub duplicate: u32,
    pub invalid: u32,
    pub failed: u32,
}

pub async fn register_local_file(
    database: &Database,
    project_id: uuid::Uuid,
    path: &Path,
) -> Result<RegistrationResult> {
    validate_regular_file(path)?;
    let normalized_identity = format!(
        "local_file:{}",
        path.to_string_lossy().trim().to_lowercase()
    );
    if let Some(source) = database
        .sources()
        .get_by_identity(project_id, &normalized_identity)
        .await?
    {
        if let Some(job) = database
            .jobs()
            .get_by_logical_key(project_id, Some(source.id), JobType::MediaIngestion)
            .await?
        {
            return Ok(RegistrationResult {
                source_id: source.id,
                job_id: job.id,
            });
        }
    }
    let source = database
        .sources()
        .create(
            project_id,
            SourceType::LocalFile,
            Some(path.to_string_lossy().as_ref()),
            None,
        )
        .await?;
    let job = database
        .jobs()
        .create(
            project_id,
            Some(source.id),
            JobType::MediaIngestion,
            0,
            Utc::now(),
        )
        .await?;
    Ok(RegistrationResult {
        source_id: source.id,
        job_id: job.id,
    })
}

pub async fn import_local_file_to_staging(
    database: &Database,
    source_id: uuid::Uuid,
    source_path: &Path,
    staging_dir: &Path,
) -> Result<(PathBuf, u64)> {
    fs::create_dir_all(staging_dir)?;
    let source = database
        .sources()
        .get(source_id)
        .await?
        .ok_or_else(|| anyhow!("source not found"))?;
    if let Some(import_metadata) = source.import_metadata.as_deref() {
        let existing = staging_path_from_metadata(import_metadata)?;
        if existing.exists() {
            let size = existing.metadata()?.len();
            return Ok((existing, size));
        }
    }
    let tmp_path = staging_dir.join(format!("{}.part", source_id));
    let final_path = staging_dir.join(format!("{}.bin", source_id));
    fs::copy(source_path, &tmp_path)?;
    fs::rename(&tmp_path, &final_path)?;
    let size = final_path.metadata()?.len();
    let metadata = format!(
        r#"{{"staged_path":"{}","bytes":{}}}"#,
        final_path.display(),
        size
    );
    let _ = database
        .sources()
        .update_import_metadata(source_id, Some(&metadata))
        .await?;
    Ok((final_path, size))
}

pub async fn register_public_url(
    database: &Database,
    project_id: uuid::Uuid,
    url: &str,
) -> Result<RegistrationResult> {
    validate_public_url(url)?;
    let normalized_identity = format!("url:{}", { normalize_public_url(url) });
    if let Some(source) = database
        .sources()
        .get_by_identity(project_id, &normalized_identity)
        .await?
    {
        if let Some(job) = database
            .jobs()
            .get_by_logical_key(project_id, Some(source.id), JobType::MediaIngestion)
            .await?
        {
            return Ok(RegistrationResult {
                source_id: source.id,
                job_id: job.id,
            });
        }
    }
    let source = database
        .sources()
        .create(project_id, SourceType::PublicUrl, Some(url), None)
        .await?;
    let job = database
        .jobs()
        .create(
            project_id,
            Some(source.id),
            JobType::MediaIngestion,
            0,
            Utc::now(),
        )
        .await?;
    Ok(RegistrationResult {
        source_id: source.id,
        job_id: job.id,
    })
}

pub async fn scan_folder(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
) -> Result<(u32, u32, u32)> {
    let root = root.canonicalize().context("folder must exist")?;
    let mut discovered = 0;
    let mut created = 0;
    let mut skipped = 0;
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !path.starts_with(&root) {
            skipped += 1;
            continue;
        }
        if !is_supported_media(path) {
            skipped += 1;
            continue;
        }
        discovered += 1;
        let normalized_identity = format!(
            "local_file:{}",
            path.to_string_lossy().trim().to_lowercase()
        );
        let existed = database
            .sources()
            .get_by_identity(project_id, &normalized_identity)
            .await?
            .is_some();
        let _ = register_local_file(database, project_id, path).await?;
        if existed {
            skipped += 1;
        } else {
            created += 1;
        }
    }
    Ok((discovered, created, skipped))
}

pub async fn import_csv_urls(
    database: &Database,
    project_id: uuid::Uuid,
    csv_path: &Path,
) -> Result<CsvImportSummary> {
    let file = fs::File::open(csv_path)?;
    let mut reader = csv::Reader::from_reader(BufReader::new(file));
    let headers = reader.headers()?.clone();
    let url_index = headers
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case("url"))
        .ok_or_else(|| anyhow!("CSV must contain a url column"))?;
    let mut summary = CsvImportSummary {
        imported: 0,
        duplicate: 0,
        invalid: 0,
        failed: 0,
    };
    for record in reader.records() {
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                summary.failed += 1;
                continue;
            }
        };
        match record
            .get(url_index)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(url) => {
                if validate_public_url(url).is_err() {
                    summary.invalid += 1;
                    continue;
                }
                let normalized_identity = format!("url:{}", normalize_public_url(url));
                let existed = database
                    .sources()
                    .get_by_identity(project_id, &normalized_identity)
                    .await?
                    .is_some();
                match register_public_url(database, project_id, url).await {
                    Ok(_) if existed => summary.duplicate += 1,
                    Ok(_) => summary.imported += 1,
                    Err(_) => summary.failed += 1,
                }
            }
            None => summary.invalid += 1,
        }
    }
    Ok(summary)
}

fn validate_public_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(())
    } else {
        Err(anyhow!("url must start with http:// or https://"))
    }
}

pub fn validate_regular_file(path: &Path) -> Result<()> {
    let metadata =
        fs::metadata(path).with_context(|| format!("path does not exist: {}", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!("path is not a regular file: {}", path.display()));
    }
    Ok(())
}

pub fn is_supported_media(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()),
        Some(ext) if matches!(ext.as_str(), "wav" | "mp3" | "m4a" | "mp4" | "mkv" | "webm" | "flac" | "ogg" | "aac")
    )
}

fn normalize_public_url(url: &str) -> String {
    let trimmed = url.trim();
    if let Some(rest) = trimmed.strip_prefix("https://youtu.be/") {
        return format!("youtube:{}", rest.trim_matches('/'));
    }
    if let Some(rest) = trimmed.strip_prefix("https://www.youtube.com/watch?v=") {
        return format!("youtube:{}", rest.split('&').next().unwrap_or(rest));
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    without_query.trim_end_matches('/').to_lowercase()
}

fn staging_path_from_metadata(metadata: &str) -> Result<PathBuf> {
    let value: serde_json::Value = serde_json::from_str(metadata)?;
    let path = value
        .get("staged_path")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("missing staged_path"))?;
    Ok(PathBuf::from(path))
}
